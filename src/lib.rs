use std::{any::TypeId, collections::HashMap, sync::Arc, thread};

use crossbeam_channel::{Receiver, TryRecvError};
use event::EventHandler;
use game_loop::{game_loop, GameLoop, Time, winit::{window::{Window, WindowBuilder}, event::Event, event_loop::EventLoop}};
use hook::{Hook, Hooks};
use parking_lot::{Condvar, Mutex};

pub use game_loop::winit as winit;

pub mod event;
pub mod hook;

#[cfg(test)]
mod tests;

pub struct Modules {
    ready: Arc<(Mutex<bool>, Condvar)>,
    exit: Arc<EventHandler<()>>,
    update: Arc<EventHandler<()>>,
    hooks: Hooks,
    hooks_constructor: Option<HashMap<TypeId, Hook>>, // Messy code, but there are no clean solutions to this problem
}

impl Modules {
    pub fn new() -> Self {
        Self {
            ready: Arc::new((Mutex::new(false), Condvar::new())),
            exit: Arc::new(EventHandler::new()),
            update: Arc::new(EventHandler::new()),
            hooks: Hooks::new(HashMap::new()),
            hooks_constructor: Option::Some(HashMap::new()),
        }
    }

    pub fn reset(&mut self) {
        *self = Modules::new();
    }

    /// The max frame time is maximum time in seconds that the render function can take before the update function starts getting called less frequently
    pub fn start<S, U, R, H>(mut self, state: S, event_loop: EventLoop<()>, window: WindowBuilder, updates_per_second: u32, max_frame_time: f64, mut update: U, mut render: R, mut events: H) -> GameLoop<(S, Self), Time, ()>
        where
            S: 'static,
            U: FnMut(&mut GameLoop<(S, Self), Time, Arc<Window>>) + 'static,
            R: FnMut(&mut GameLoop<(S, Self), Time, Arc<Window>>) + 'static,
            H: FnMut(&mut GameLoop<(S, Self), Time, Arc<Window>>, &Event<'_, ()>) + 'static,
    {
        // Construct hooks from the hooks constructor
        self.hooks.reload(self.hooks_constructor.take().unwrap());

        {
            *self.ready.0.lock() = true;
            self.ready.1.notify_all();
        }

        let window = Arc::new(window.build(&event_loop).unwrap());

        game_loop(event_loop, window, (state, self), updates_per_second, max_frame_time,
        move |g| {
            let _ = g.game.1.update.emit(&());
            update(g)
        }, move |g| {
            render(g)
        }, move |g, e| {
            events(g, e)
        })
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

                module.run(exit_receiver, update_receiver, hooks)
            })
            .unwrap();
    }
}

pub trait Module {
    fn run(self, exit: Exit, update: Update, hooks: Hooks);
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
    pub fn can(&self) -> bool {
        match self.0.try_recv() {
            Ok(_) => true,
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => false,
        }
    }
}
