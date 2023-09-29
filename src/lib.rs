use std::{any::TypeId, collections::HashMap, sync::Arc, thread};

use crossbeam_channel::{Receiver, TryRecvError};
use event::EventHandler;
use hook::{Hook, Hooks};
use parking_lot::{Condvar, Mutex};

pub mod event;
pub mod hook;

#[cfg(test)]
mod tests;

pub struct Modules {
    ready: Arc<(Mutex<bool>, Condvar)>,
    exit: EventHandler<()>,
    update: EventHandler<()>,
    hooks: Hooks,
    hooks_constructor: Option<HashMap<TypeId, Hook>>, // Messy code, but there are no clean solutions to this problem
}

impl Modules {
    pub fn new() -> Self {
        Self {
            ready: Arc::new((Mutex::new(false), Condvar::new())),
            exit: EventHandler::new(),
            update: EventHandler::new(),
            hooks: Hooks::new(HashMap::new()),
            hooks_constructor: Option::Some(HashMap::new()),
        }
    }

    pub fn reset(&mut self) {
        *self = Modules::new();
    }

    #[cfg(not(feature = "window"))]
    pub fn start<S, U, R>(
        mut self,
        state: S,
        updates_per_second: u32,
        max_frame_time: f64,
        mut update: U,
        mut render: R,
    ) -> game_loop::GameLoop<(S, Self), game_loop::Time, ()>
    where
        S: 'static,
        U: FnMut(&mut game_loop::GameLoop<(S, Self), game_loop::Time, ()>)
            + 'static,
        R: FnMut(&mut game_loop::GameLoop<(S, Self), game_loop::Time, ()>)
            + 'static,
    {
        use game_loop::game_loop;

        // Construct hooks from the hooks constructor
        self.hooks.reload(self.hooks_constructor.take().unwrap());

        {
            *self.ready.0.lock() = true;
            self.ready.1.notify_all();
        }

        game_loop(
            (state, self),
            updates_per_second,
            max_frame_time,
            move |g| {
                let _ = g.game.1.update.emit(&());
                update(g)
            },
            move |g| {
                render(g)
            },
        )
    }

    /// The max frame time is maximum time in seconds that the render function can take before the update function starts getting called less frequently
    #[cfg(feature = "window")]
    pub fn start<S, U, R, H>(
        mut self,
        state: S,
        event_loop: winit::event_loop::EventLoop<()>,
        window: winit::window::WindowBuilder,
        updates_per_second: u32,
        max_frame_time: f64,
        mut update: U,
        mut render: R,
        mut events: H,
    ) -> game_loop::GameLoop<(S, Self), game_loop::Time, ()>
    where
        S: 'static,
        U: FnMut(&mut game_loop::GameLoop<(S, Self), game_loop::Time, Arc<winit::window::Window>>)
            + 'static,
        R: FnMut(&mut game_loop::GameLoop<(S, Self), game_loop::Time, Arc<winit::window::Window>>)
            + 'static,
        H: FnMut(
                &mut game_loop::GameLoop<(S, Self), game_loop::Time, Arc<winit::window::Window>>,
                &winit::event::Event<'_, ()>,
            ) + 'static,
    {
        // Construct hooks from the hooks constructor
        self.hooks.reload(self.hooks_constructor.take().unwrap());

        {
            *self.ready.0.lock() = true;
            self.ready.1.notify_all();
        }

        let window = Arc::new(window.build(&event_loop).unwrap());

        crate::windowed::game_loop(
            event_loop,
            window,
            (state, self),
            updates_per_second,
            max_frame_time,
            move |g| {
                let _ = g.game.1.update.emit(&());
                update(g)
            },
            move |g| {
                render(g)
            },
            move |g, e| {
                events(g, e)
            },
        )
    }

    pub fn add_module<M: Module + Send + 'static>(&mut self, mut module: M) {
        // Clone variables to be passed to new thread
        let ready = self.ready.clone();
        let exit = self.exit.clone();
        let update = self.update.clone();
        let hooks = self.hooks.clone();

        self.hooks_constructor
            .as_mut()
            .unwrap()
            .insert(TypeId::of::<M>(), module.hook());

        // Subscribe to the shutdown reciever for every thread
        let exit_receiver = Exit(exit.subscribe());
        let update_receiver = Update(update.subscribe());

        thread::Builder::new()
            .name(std::any::type_name::<M>().into())
            .spawn(move || {
                // Wait for all modules to be added before starting thread
                let mut started = ready.0.lock();
                if !*started {
                    ready.1.wait(&mut started);
                }
                drop(started);

                module.start(&hooks);
                loop {
                    if exit_receiver.should_exit() {
                        break;
                    }

                    if update_receiver.tick() {
                        module.update(&hooks);
                    }

                    module.render(&hooks);
                }
            })
            .unwrap();
    }
}

pub trait Module {
    fn start(&mut self, hooks: &Hooks);
    fn render(&mut self, hook: &Hooks);
    fn update(&mut self, hooks: &Hooks);
    fn hook(&mut self) -> Hook;
}

pub struct Exit(Receiver<()>);

impl Exit {
    pub fn should_exit(&self) -> bool {
        match self.0.try_recv() {
            Ok(_) => true,
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => true,
        }
    }
}

pub struct Update(Receiver<()>);

impl Update {
    pub fn tick(&self) -> bool {
        match self.0.try_recv() {
            Ok(_) => true,
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => false,
        }
    }
}

#[cfg(feature="window")]
mod windowed {
    use std::sync::Arc;
    use game_loop::{GameLoop, Time};
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::{ControlFlow, EventLoop};
    use winit::window::Window;

    pub fn game_loop<G, U, R, H, T>(event_loop: EventLoop<T>, window: Arc<Window>, game: G, updates_per_second: u32, max_frame_time: f64, mut update: U, mut render: R, mut handler: H) -> !
        where G: 'static,
              U: FnMut(&mut GameLoop<G, Time, Arc<Window>>) + 'static,
              R: FnMut(&mut GameLoop<G, Time, Arc<Window>>) + 'static,
              H: FnMut(&mut GameLoop<G, Time, Arc<Window>>, &Event<'_, T>) + 'static,
              T: 'static,
    {
        let mut game_loop = GameLoop::new(game, updates_per_second, max_frame_time, window);

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            // Forward events to existing handlers.
            handler(&mut game_loop, &event);

            match event {
                Event::RedrawRequested(_) => {
                    if !game_loop.next_frame(&mut update, &mut render) {
                        *control_flow = ControlFlow::Exit;
                    }
                },
                Event::MainEventsCleared => {
                    game_loop.window.request_redraw();
                },
                Event::WindowEvent { event: WindowEvent::Occluded(occluded), .. } => {
                    game_loop.window_occluded = occluded;
                },
                _ => {},
            }
        })
    }
}