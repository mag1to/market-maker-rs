use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;

use crossbeam_channel::{unbounded, Receiver, Sender};

#[derive(Error, Debug)]
pub enum PubSubError {
    #[error("publish error: {0:?}")]
    PublishError(Vec<SubscriptionId>),
    #[error("disconnected: {0:?}")]
    Disconnected(SubscriptionId),
    #[error("subscription error: {0:?}")]
    SubscriptionError(SubscriptionId),
}

pub type PubSubResult<T> = Result<T, PubSubError>;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubscriptionId(u64);

impl From<u64> for SubscriptionId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<SubscriptionId> for u64 {
    fn from(id: SubscriptionId) -> Self {
        id.0
    }
}

#[derive(Clone)]
pub struct PubSub<T>(Arc<Mutex<PubSubInner<T>>>);

impl<T> Default for PubSub<T>
where
    T: Clone,
{
    fn default() -> Self {
        Self(Arc::new(Mutex::new(PubSubInner::default())))
    }
}

impl<T> PubSub<T>
where
    T: Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self) -> Subscription<T> {
        let mut guard = self.0.lock().unwrap();
        guard.subscribe()
    }

    pub fn unsubscribe(&self, id: &SubscriptionId) {
        let mut guard = self.0.lock().unwrap();
        guard.unsubscribe(id)
    }

    pub fn publish(&self, message: T) {
        let mut guard = self.0.lock().unwrap();
        guard.publish(message)
    }
}

struct PubSubInner<T> {
    nonce: u64,
    subscribers: HashMap<SubscriptionId, (Sender<T>, Arc<AtomicBool>)>,
}

impl<T> Default for PubSubInner<T> {
    fn default() -> Self {
        Self {
            nonce: 0,
            subscribers: HashMap::new(),
        }
    }
}

impl<T> PubSubInner<T>
where
    T: Clone,
{
    fn subscribe(&mut self) -> Subscription<T> {
        let id = self.nonce.into();
        let exit = Arc::new(AtomicBool::new(false));

        let (sender, receiver) = unbounded();
        let subscription = Subscription::new(id, exit.clone(), receiver);

        self.subscribers.insert(id, (sender, exit));
        self.nonce = self.nonce.checked_add(1).expect("overflow");

        subscription
    }

    fn unsubscribe(&mut self, id: &SubscriptionId) {
        if let Some((_, exit)) = self.subscribers.get(id) {
            exit.store(true, Ordering::Relaxed);
        }
        self.subscribers.remove(id);
    }

    fn publish(&mut self, message: T) {
        let mut disconnected = Vec::new();
        match self.subscribers.len() {
            0 => return,
            1 => {
                let (id, (sender, _)) = self.subscribers.iter().next().unwrap();

                if sender.send(message).is_err() {
                    disconnected.push(*id);
                }
            }
            n => {
                let mut last = None;
                for (i, (id, (sender, _))) in self.subscribers.iter().enumerate() {
                    if i == n - 1 {
                        last = Some(id);
                        break;
                    }

                    if sender.send(message.clone()).is_err() {
                        disconnected.push(*id);
                    }
                }

                let id = last.expect("must exists");
                let (sender, _) = self.subscribers.get(id).expect("must exists");
                if sender.send(message).is_err() {
                    disconnected.push(*id);
                }
            }
        }

        for id in &disconnected {
            self.unsubscribe(id);
        }
    }
}

pub struct Subscription<T> {
    id: SubscriptionId,
    exit: Arc<AtomicBool>,
    receiver: Receiver<T>,
}

impl<T> Subscription<T> {
    fn new(id: SubscriptionId, exit: Arc<AtomicBool>, receiver: Receiver<T>) -> Self {
        Self { id, exit, receiver }
    }

    pub fn id(&self) -> SubscriptionId {
        self.id
    }

    pub fn disconnected(&self) -> bool {
        self.exit.load(Ordering::Relaxed)
    }

    pub fn recv(&self) -> PubSubResult<T> {
        self.check_connection()?;
        self.receiver
            .recv()
            .map_err(|_| PubSubError::SubscriptionError(self.id()))
    }

    pub fn iter(&self) -> PubSubResult<impl Iterator<Item = T> + '_> {
        self.check_connection()?;
        Ok(self.receiver.iter())
    }

    pub fn try_iter(&self) -> PubSubResult<impl Iterator<Item = T> + '_> {
        self.check_connection()?;
        Ok(self.receiver.try_iter())
    }

    pub fn as_receiver(&self) -> &Receiver<T> {
        &self.receiver
    }

    fn check_connection(&self) -> PubSubResult<()> {
        if self.disconnected() {
            return Err(PubSubError::Disconnected(self.id()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pubsub() {
        let pubsub: PubSub<u64> = PubSub::new();

        let sub1 = pubsub.subscribe();

        pubsub.publish(1);
        pubsub.publish(2);

        assert_eq!(sub1.try_iter().unwrap().collect::<Vec<_>>(), vec![1, 2]);

        let sub2 = pubsub.subscribe();

        pubsub.publish(3);
        pubsub.publish(4);

        assert_eq!(sub1.try_iter().unwrap().collect::<Vec<_>>(), vec![3, 4]);
        assert_eq!(sub2.try_iter().unwrap().collect::<Vec<_>>(), vec![3, 4]);

        assert!(!sub1.disconnected());
        assert!(!sub2.disconnected());

        pubsub.unsubscribe(&sub1.id());

        assert!(sub1.disconnected());
        assert!(!sub2.disconnected());

        pubsub.publish(5);
        pubsub.publish(6);

        assert!(sub1.try_iter().is_err());
        assert_eq!(sub2.try_iter().unwrap().collect::<Vec<_>>(), vec![5, 6]);

        drop(sub2);

        pubsub.publish(7);

        assert!(pubsub.0.lock().unwrap().subscribers.is_empty());
    }
}
