#![allow(unused)]
use chrono;
use linked_hash_map::LinkedHashMap;

pub mod traits;
use traits::Memoize;

const DEFAULT_TTL_DURATION: u64 = 10;
const DEFAULT_CACHE_CAPACITY: usize = 8;

#[derive(PartialEq)]
enum RevalidationAction {
    EXPIRE,
    REVALIDATE,
}

enum TtlSetting {
    Blocking,
    Expire,
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

pub struct CacheNode<K, V, C> {
    cache: linked_hash_map::LinkedHashMap<K, V>,
    ttl: TtlOptions,
    capacity: usize,
    calculation: Option<C>,
}

impl<K, V, C> CacheNode<K, V, C>
where
    K: Copy + Eq + std::hash::Hash,
    V: Copy,
    C: Fn(K) -> V,
{
    pub fn capacity(mut self, entries: usize) -> Self {
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
            Some(e ) => Ok(e),
            None => Err(())
        }
    }

    pub fn expires(mut self, seconds: u64) -> Self {
        self.ttl.expiration = Some(chrono::Utc::now() + chrono::Duration::seconds(seconds as i64));
        self.ttl.revalidation.duration = seconds;
        self
    }

    pub fn new() -> CacheNode<K, V, C> {
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
            calculation: None,
        }
    }

    pub fn revalidate(mut self, status: bool) -> Self {
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

    pub fn with_memo(mut self, calculation: C) -> Self {
        self.calculation = Some(calculation);
        self
    }
}
impl<K, V, C> Memoize<K, V> for CacheNode<K, V, C>
where
    K: Copy + Eq + std::hash::Hash,
    V: Copy,
    C: Fn(K) -> V,
{
    fn memoize(&mut self, args: K) -> V {
        let v = (*self.calculation.as_ref().unwrap())(args);
        self.cache.insert(args, v);
        v
    }

    fn value(&mut self, args: K) -> V {
        match self.cache.get(&args) {
            Some(v) => {
                if let Ok(_) = self.validate_expiration() {
                    *v
                } else {
                    if self.ttl.revalidation.action == RevalidationAction::REVALIDATE {
                        self.ttl.expiration = Some(
                            chrono::Utc::now()
                                + chrono::Duration::seconds(self.ttl.revalidation.duration as i64),
                        );
                        *v
                    } else {
                        self.ttl.expiration = Some(
                            chrono::Utc::now()
                                + chrono::Duration::seconds(self.ttl.revalidation.duration as i64),
                        );
                        self.cache.clear();
                        self.memoize(args)
                    }
                }
            }
            None => {
                if let Ok(_) = self.check_capacity() {
                    self.memoize(args)
                } else {
                    match self.clean_up() {
                        Ok(_) => self.memoize(args),
                        Err(_) => {
                            self.cache.clear();
                            panic!("Cache buffer overflow")
                        }
                    }
                }
            }
        }
    }
}
