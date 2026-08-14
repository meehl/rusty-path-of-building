use std::rc::Rc;

use crate::fonts::Layout;

struct CachedLayout {
    layout: Rc<Layout>,
    // used to identify unused layouts
    generation: u32,
}

#[derive(Default)]
pub struct LayoutCache {
    cache: nohash_hasher::IntMap<u64, CachedLayout>,
    current_generation: u32,
}

impl LayoutCache {
    pub fn get(&mut self, hash: u64) -> Option<Rc<Layout>> {
        match self.cache.entry(hash) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                let cached = entry.into_mut();
                cached.generation = self.current_generation;
                Some(Rc::clone(&cached.layout))
            }
            std::collections::hash_map::Entry::Vacant(_) => None,
        }
    }

    pub fn insert(&mut self, hash: u64, layout: Rc<Layout>) {
        self.cache.insert(
            hash,
            CachedLayout {
                generation: self.current_generation,
                layout,
            },
        );
    }

    /// Removes unused layouts
    pub fn flush(&mut self) {
        self.cache
            .retain(|_key, cached| cached.generation == self.current_generation);
        self.current_generation = self.current_generation.wrapping_add(1);
    }
}
