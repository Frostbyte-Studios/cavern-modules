use std::{collections::HashMap, any::{TypeId, Any}, sync::Arc};

use arc_swap::ArcSwap;
use crossbeam_channel::Receiver;

use crate::event::{InnerEventHandler, EventHandler};

#[derive(Clone)]
pub struct Hooks(Arc<ArcSwap<HashMap<TypeId, Hook>>>);

impl Hooks {
    pub fn new(modules: HashMap<TypeId, Hook>) -> Self {
        Self(Arc::new(ArcSwap::from_pointee(modules)))
    }

    pub fn get<T: 'static, V>(&self, value: impl FnOnce(&Hook) -> V) -> Option<V> {
        if let Some(hook) = self.0.load().get(&TypeId::of::<T>()) {
            return Some(value(hook));
        }
        None
    }

    pub fn reload(&self, modules: HashMap<TypeId, Hook>) {
        self.0.store(Arc::new(modules));
    }

    // Returns reference to inner HashMap
    /*pub(crate) fn inner(&self) -> Arc<HashMap<TypeId, Hook>> {
        self.0.load().clone()
    }*/
}

pub struct Hook(Arc<dyn Any + Send + Sync>, HashMap<&'static str, Arc<dyn Any + Send + Sync>>);

impl Hook {
    pub fn new<T: Send + Sync + 'static>(state: T) -> Self {
        Self(Arc::new(state), HashMap::new())
    }

    pub fn with<T: Send + 'static>(mut self, id: &'static str, handler: EventHandler<T>) -> Self {
        self.1.insert(id, handler.0);
        self
    }

    pub fn add<T: Send + 'static>(&mut self, id: &'static str, handler: EventHandler<T>) {
        self.1.insert(id, handler.0);
    }

    pub fn get<T: Clone + 'static>(&self, id: &'static str) -> Option<Receiver<T>> {
        let any = self.1.get(&id)?;
        let handler = any.downcast_ref::<InnerEventHandler<T>>()?;
        Some(handler.subscribe())
    }

    pub fn state<T: 'static>(&self) -> Option<&T> {
        self.0.downcast_ref::<T>()
    }
}
