use std::{sync::Arc, ops::Deref};

use crossbeam_channel::{Sender, Receiver, TrySendError};
use parking_lot::RwLock;

#[derive(Clone, Default)]
pub struct EventHandler<T>(Arc<InnerEventHandler<T>>);

impl<T> Deref for EventHandler<T> {
    type Target = Arc<InnerEventHandler<T>>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Clone> EventHandler<T> {
    pub fn new() -> Self {
        Self(Arc::new(InnerEventHandler::new()))
    }
}

#[derive(Default)]
pub struct InnerEventHandler<T>(RwLock<Vec<Sender<T>>>);

impl<T: Clone> InnerEventHandler<T> {
    pub fn new() -> Self {
        Self(RwLock::new(Vec::new()))
    }

    pub fn emit(&self, data: &T) -> Result<(), TrySendError<T>> {
        let guard = self.0.read();
        Ok(emit(&guard, data)?)
    }

    pub fn try_emit(&self, data: &T) -> Result<(), EmitError<T>> {
        let guard = self.0.try_read().ok_or(EmitError::LockError)?;
        Ok(emit(&guard, data)?)
    }

    pub fn subscribe(&self) -> Receiver<T> {
        let mut guard = self.0.write();
        let (sender, receiver) = crossbeam_channel::unbounded();
        guard.push(sender);
        receiver
    }
}

fn emit<T: Clone>(senders: &Vec<Sender<T>>, data: &T) -> Result<(), TrySendError<T>> {
    for s in senders {
        s.try_send(data.clone())?;
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum EmitError<T> {
    #[error("Could not aquire event lock; someone could be subscribing to the event.")]
    LockError,

    #[error(transparent)]
    TrySendError(#[from] TrySendError<T>),
}