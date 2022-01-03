#![allow(unused)]
use chrono;
use linked_hash_map::{LinkedHashMap};

pub mod traits;
use crate::traits::{CacheCapacityController, CacheExpirationController};

const DEFAULT_TTL_DURATION: u64 = 10;
const DEFAULT_CACHE_CAPACITY: usize = 8;

#[derive(PartialEq)]
enum RevalidationAction {
    EXPIRE,
    REVALIDATE,
}

enum TtlSetting {
    Blocking,
    // TODO: add async/await support
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
    calculation: Box<C>,
}

pub struct CacheNode<K, V, S> {
    pub cache: linked_hash_map::LinkedHashMap<Box<K>, Box<V>>,
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
        if let Ok(_) = self.check_capacity() {
            self.cache.insert(Box::new(key), Box::new(value));
        } else {
            match self.clean_up() {
                Ok(_) => {
                    self.cache.insert(Box::new(key), Box::new(value));
                }
                Err(_) => {
                    self.cache.clear();
                    self.cache.insert(Box::new(key), Box::new(value));
                }
            }
        }
    }

    pub fn new() -> CacheNode<K, V, Initialized> {
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
            controller: Memoized { calculation: Box::new(calculation) },
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
        self.cache.insert(Box::new(*args), Box::new(v));
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
                if let Ok(_) = self.check_capacity() {
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

impl<K, V, S> CacheCapacityController<K, V> for CacheNode<K, V, S>
    where
        K: Eq + std::hash::Hash,
{
    fn capacity(mut self, entries: usize) -> Self {
        self.capacity = entries;
        self
    }

    fn check_capacity(&self) -> Result<(), ()> {
        if self.cache.len() < self.capacity {
            Ok(())
        } else {
            Err(())
        }
    }

    fn clean_up(&mut self) -> Result<(K, V), ()> {
        match self.cache.pop_front() {
            Some(e) => Ok((*e.0, *e.1)),
            None => Err(()),
        }
    }
}

impl<K, V, S> CacheExpirationController for CacheNode<K, V, S> {
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
        let t = chrono::Utc::now();
        match self.ttl.expiration {
            Some(expiration) => {
                if t > expiration {
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
    pub buffer: Vec<Box<dyn std::any::Any>>,
}