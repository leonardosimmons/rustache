#![allow(unused)]

use std::rc::Rc;
use std::any::Any;

use chrono;
use linked_hash_map::LinkedHashMap;

const DEFAULT_TTL_DURATION: u64 = 10;
const DEFAULT_CACHE_CAPACITY: usize = 8;

#[derive(PartialEq)]
enum RevalidationAction {
    EXPIRE,
    REVALIDATE,
}

enum TtlSetting {
    Blocking,
    // TODO: add SWR support
    Swr,
}

struct RevalidationSettings {
    action: RevalidationAction,
    duration: u64,
    setting: TtlSetting,
}

struct TtlOptions {
    revalidation: RevalidationSettings,
    expiration: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct Initialized;
pub struct Memoized<C> {
    calculation: Rc<C>,
}

pub struct CacheNode<K, V, S> {
    cache: linked_hash_map::LinkedHashMap<Rc<K>, Rc<V>>,
    ttl: TtlOptions,
    capacity: usize,
    controller: S,
}

impl<K, V> CacheNode<K, V, Initialized>
where
    K: Eq + std::hash::Hash,
    V: Copy,
{
    pub fn get(&mut self, key: K) -> Option<V> {
        match self.cache.get(&key) {
            Some(v) => {
                if let Ok(_) = self.validate_expiration() {
                    Some(**v)
                } else {
                    if self.ttl.revalidation.action == RevalidationAction::REVALIDATE {
                        self.ttl.expiration = Some(
                            chrono::Utc::now()
                                + chrono::Duration::seconds(self.ttl.revalidation.duration as i64),
                        );
                        Some(**v)
                    } else {
                        self.ttl.expiration = Some(
                            chrono::Utc::now()
                                + chrono::Duration::seconds(self.ttl.revalidation.duration as i64),
                        );
                        self.cache.clear();
                        None
                    }
                }
            }
            None => None,
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if let true = self.capacity_check() {
            self.cache.insert(Rc::new(key), Rc::new(value));
        } else {
            match self.clean_up() {
                Ok(_) => {
                    self.cache.insert(Rc::new(key), Rc::new(value));
                }
                Err(_) => {
                    self.cache.clear();
                    self.cache.insert(Rc::new(key), Rc::new(value));
                }
            }
        }
    }

    fn new() -> CacheNode<K, V, Initialized> {
        CacheNode {
            cache: linked_hash_map::LinkedHashMap::new(),
            ttl: TtlOptions {
                revalidation: RevalidationSettings {
                    action: RevalidationAction::EXPIRE,
                    duration: DEFAULT_TTL_DURATION,
                    setting: TtlSetting::Blocking,
                },
                expiration: None,
            },
            capacity: DEFAULT_CACHE_CAPACITY,
            controller: Initialized,
        }
    }

    pub fn with_memo<C>(mut self, calculation: C) -> CacheNode<K, V, Memoized<C>> {
        CacheNode {
            cache: self.cache,
            capacity: self.capacity,
            ttl: self.ttl,
            controller: Memoized {
                calculation: Rc::new(calculation),
            },
        }
    }
}

impl<K, V, C> CacheNode<K, V, Memoized<C>>
where
    K: Copy + Eq + std::hash::Hash,
    V: Copy,
    C: Fn(K) -> V,
{
    pub fn memoize(&mut self, args: &K) -> V {
        let v = (*self.controller.calculation)(*args);
        self.cache.insert(Rc::new(*args), Rc::new(v));
        v
    }

    pub fn value(&mut self, args: K) -> Option<V> {
        match self.cache.get(&args) {
            Some(v) => {
                if let Ok(_) = self.validate_expiration() {
                    Some(**v)
                } else {
                    if self.ttl.revalidation.action == RevalidationAction::REVALIDATE {
                        self.ttl.expiration = Some(
                            chrono::Utc::now()
                                + chrono::Duration::seconds(self.ttl.revalidation.duration as i64),
                        );
                        Some(**v)
                    } else {
                        self.ttl.expiration = Some(
                            chrono::Utc::now()
                                + chrono::Duration::seconds(self.ttl.revalidation.duration as i64),
                        );
                        self.cache.clear();
                        Some(self.memoize(&args))
                    }
                }
            }
            None => {
                if let true = self.capacity_check() {
                    Some(self.memoize(&args))
                } else {
                    match self.clean_up() {
                        Ok(_) => Some(self.memoize(&args)),
                        Err(_) => {
                            self.cache.clear();
                            Some(self.memoize(&args))
                        }
                    }
                }
            }
        }
    }
}

impl<K, V, S> CapacityController<K, V> for CacheNode<K, V, S>
where
    K: Eq + std::hash::Hash,
{
    fn capacity(mut self, entries: usize) -> Self {
        self.capacity = entries;
        self
    }

    fn capacity_check(&self) -> bool {
        self.cache.len() < self.capacity
    }

    fn clean_up(&mut self) -> Result<(), ()> {
        if let Some(e) = self.cache.pop_front() {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl<K, V, S> ExpirationController for CacheNode<K, V, S> {
    fn expires(mut self, seconds: u64) -> Self {
        self.ttl.expiration = Some(chrono::Utc::now() + chrono::Duration::seconds(seconds as i64));
        self.ttl.revalidation.duration = seconds;
        self
    }

    fn revalidate(mut self, status: bool) -> Self {
        if status == true {
            self.ttl.revalidation.action = RevalidationAction::REVALIDATE;
            self
        } else {
            self.ttl.revalidation.action = RevalidationAction::EXPIRE;
            self
        }
    }

    fn validate_expiration(&self) -> Result<(), &str> {
        match self.ttl.expiration {
            Some(expiration) => {
                if chrono::Utc::now() > expiration {
                    Err("expired")
                } else {
                    Ok(())
                }
            }
            None => Ok(()),
        }
    }
}

pub struct Cache {
    pub buffer: Vec<Rc<dyn std::any::Any>>,
}
impl Cache {
    pub fn new() -> Self {
        Cache { buffer: vec![] }
    }

    pub fn new_node<K: Eq + std::hash::Hash, V: Copy>() -> CacheNode<K, V, Initialized> {
        CacheNode::new()
    }

    pub fn push<N>(&mut self, node: &'static N) {
        self.buffer.push(Rc::new(node));
    }

    pub fn remove<N: std::cmp::PartialEq + 'static>(&mut self, node: N) {
        let index = self
            .buffer
            .iter()
            .position(|n| {
                *Rc::clone(&Rc::new(n.downcast_ref::<N>().unwrap())) == *Rc::clone(&Rc::new(&node))
            })
            .unwrap();

        self.buffer.remove(index);
    }
}

pub trait CapacityController<K, V>
    where
        K: Eq + std::hash::Hash,
{
    fn capacity(self, entries: usize) -> Self;
    fn capacity_check(&self) -> bool;
    fn clean_up(&mut self) -> Result<(), ()>;
}

pub trait ExpirationController {
    fn expires(self, seconds: u64) -> Self;
    fn revalidate(self, status: bool) -> Self;
    fn validate_expiration(&self) -> Result<(), &str>;
}
