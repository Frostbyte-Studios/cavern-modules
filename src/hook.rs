use std::{collections::HashMap, any::{TypeId, Any}, sync::Arc};

use arc_swap::ArcSwap;

use crate::event::EventHandler;

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

pub struct Hook(HashMap<&'static str, Box<dyn Any + Send + Sync>>);

impl Hook {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add<T: Send + 'static>(&mut self, id: &'static str, handler: EventHandler<T>) {
        self.0.insert(id, Box::new(handler));
    }

    pub fn get<T: 'static>(&self, id: &'static str) -> Option<&EventHandler<T>> {
        let any = self.0.get(&id)?;
        any.downcast_ref::<EventHandler<T>>()
    }
}
