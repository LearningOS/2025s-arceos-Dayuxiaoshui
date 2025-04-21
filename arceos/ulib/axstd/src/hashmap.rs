use core::hash::Hasher;
#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::hash::Hash;

#[cfg(feature = "alloc")]
pub struct HashMap<K,V> {
    data : Vec<Vec<(K,V)>>,
    capacity :usize,
}

#[derive(Default)]
struct MyHahser(u64) ;


impl Hasher for MyHahser {
    fn finish(&self) -> u64 {
        self.0  
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = self.0.wrapping_mul(31).wrapping_add(*byte as u64);
        }
    }
}
#[cfg(feature = "alloc")]
impl<K: Hash + Eq, V> HashMap<K, V> {
    pub fn new() -> Self {
        let capacity = 200 ;
        let mut data = Vec::new();
        for _ in 0..capacity {
            data.push(Vec::new());
        }

        HashMap {
            data,
            capacity,
        }
    }

    fn hash<Q: ?Sized + Hash>(&self, key: &Q) -> usize {
        let mut hasher = MyHahser::default();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.capacity
    }

    pub fn insert(&mut self, key: K, value: V) {
       
        let idx = self.hash(&key);
        for &mut (ref existing_key, ref mut existing_value) in &mut self.data[idx] {
            if existing_key == &key {
                *existing_value = value;
                return;
            }
        }

        self.data[idx].push((key, value));
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let idx = self.hash(key);
        for (existing_key, value) in &self.data[idx] {
            if existing_key == key {
                return Some(value);
            }
        }
        None
    }
    pub fn iter(&self) -> HashMapIterator<K, V> {
        HashMapIterator::<K, V> {
            map: self,
            bucket_index: 0,
            item_index: 0,
            _marker: PhantomData,
        }
    }
}
#[cfg(feature = "alloc")]
use core::marker::PhantomData;
#[cfg(feature = "alloc")]
pub struct HashMapIterator<'a, K, V> {
    map: &'a HashMap<K, V>,
    bucket_index: usize,
    item_index: usize,
    _marker: PhantomData<(K, V)>,
}
#[cfg(feature = "alloc")]
impl<'a, K, V> Iterator for HashMapIterator<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while self.bucket_index < self.map.capacity {
            let bucket = &self.map.data[self.bucket_index];
            if self.item_index < bucket.len() {
                let item = &bucket[self.item_index];
                self.item_index += 1;
                return Some((&item.0, &item.1));
            } else {
                self.bucket_index += 1;
                self.item_index = 0;
            }
        }
        None
    }
}