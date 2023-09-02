use std::{thread, sync::Arc, collections::HashMap, any::TypeId};

use crossbeam_channel::{Receiver, TryRecvError};
use event::EventHandler;
use hook::{Hooks, Hook};
use parking_lot::{Mutex, Condvar};

pub mod event;
pub mod hook;

#[cfg(test)]
mod tests;

pub struct Modules {
    ready: Arc<(Mutex<bool>, Condvar)>,
    exit: Arc<EventHandler<()>>,
    hooks: Hooks,
    hooks_constructor: Option<HashMap<TypeId, Hook>>, // Messy code, but there are no clean solutions to this problem
}

impl Modules {
    pub fn new() -> Self {
        Self {
            ready: Arc::new((Mutex::new(false), Condvar::new())),
            exit: Arc::new(EventHandler::new()),
            hooks: Hooks::new(HashMap::new()),
            hooks_constructor: Option::Some(HashMap::new()),
        }
    }

    pub fn reset(&mut self) {
        *self = Modules::new();
    }

    pub fn start(&mut self) {
        // Construct hooks from the hooks constructor
        self.hooks.reload(self.hooks_constructor.take().unwrap());

        let mut started = self.ready.0.lock();
        *started = true;
        self.ready.1.notify_all();
    }

    pub fn add_module<M: Module + Send + 'static>(&mut self, mut module: M) {
        // Clone variables to be passed to new thread
        let ready = self.ready.clone();
        let exit = self.exit.clone();
        let hooks = self.hooks.clone();

        self.hooks_constructor.as_mut().unwrap().insert(TypeId::of::<M>(), module.hook());

        // Subscribe to the shutdown reciever for every thread
        let receiver = Exit(exit.subscribe());

        thread::Builder::new().name(std::any::type_name::<M>().into()).spawn(move || {
            // Wait for all modules to be added before starting thread
            let mut started = ready.0.lock();
            if !*started {
                ready.1.wait(&mut started);
            }
            drop(started);
            
            module.run(receiver, hooks)
        }).unwrap();
    }
}

pub trait Module {
    fn run(self, exit: Exit, hooks: Hooks);
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