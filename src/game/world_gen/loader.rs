//! Streams chunks in and out around the viewer. Terrain is generated on worker threads,
//! so the game thread only ever inserts finished chunks.

use std::{
    cmp::Reverse,
    collections::HashSet,
    sync::{
        Arc, Condvar, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

use crate::game::world_gen::{
    chunk::Chunk,
    world::{ChunkKey, View, World},
};

/// Chunks generated beyond the render distance, so every chunk that gets meshed
/// already has its neighbours to cull against.
const LOAD_MARGIN: i32 = 1;
/// Chunks are only unloaded this far out, so crossing a chunk border back and forth doesn't churn.
const KEEP_MARGIN: i32 = 2;

type Generator = dyn Fn(ChunkKey) -> Chunk + Send + Sync;

struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
}

#[derive(Default)]
struct Queue {
    /// Farthest first, so `pop()` hands out the nearest.
    wanted: Vec<ChunkKey>,
    shutdown: bool,
}

impl Shared {
    fn take_wanted(&self) -> Vec<ChunkKey> {
        std::mem::take(&mut self.queue.lock().unwrap().wanted)
    }

    fn set_wanted(&self, wanted: Vec<ChunkKey>) {
        self.queue.lock().unwrap().wanted = wanted;
        self.wake.notify_all();
    }
}

pub struct ChunkLoader {
    shared: Arc<Shared>,
    results: Receiver<(ChunkKey, Chunk)>,
    /// Requested and not yet received: waiting in the queue or being generated.
    in_flight: HashSet<ChunkKey>,
    last_center: Option<ChunkKey>,
    workers: Vec<JoinHandle<()>>,
}

/// Leaves a core for the game thread, and a few for the driver.
pub fn default_workers() -> usize {
    thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(1)).clamp(1, 4)
}

impl ChunkLoader {
    pub fn new(workers: usize, generate: impl Fn(ChunkKey) -> Chunk + Send + Sync + 'static) -> Self {
        let shared = Arc::new(Shared { queue: Mutex::new(Queue::default()), wake: Condvar::new() });
        let generate: Arc<Generator> = Arc::new(generate);
        let (sender, results) = mpsc::channel();

        let workers = (0..workers.max(1))
            .map(|i| {
                let (shared, generate, sender) = (shared.clone(), generate.clone(), sender.clone());
                thread::Builder::new()
                    .name(format!("chunk-worker-{i}"))
                    .spawn(move || worker_loop(&shared, &*generate, &sender))
                    .expect("failed to spawn chunk worker")
            })
            .collect();

        ChunkLoader { shared, results, in_flight: HashSet::new(), last_center: None, workers }
    }

    /// Chunks requested but not yet loaded.
    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// Call once per frame. Inserts at most `max_inserts` finished chunks into `world`.
    pub fn update(&mut self, world: &mut World, view: &View, max_inserts: usize) {
        if self.last_center != Some(view.center()) {
            self.last_center = Some(view.center());
            world.unload_beyond(view, KEEP_MARGIN);
            self.request_missing(world, view);
        }

        for (key, chunk) in self.results.try_iter().take(max_inserts) {
            self.in_flight.remove(&key);
            if view.within(key, KEEP_MARGIN) {
                world.insert(key, chunk);
            }
        }
    }

    /// Replaces the work queue, so chunks the player has already left behind are never generated.
    fn request_missing(&mut self, world: &World, view: &View) {
        for key in self.shared.take_wanted() {
            self.in_flight.remove(&key);
        }

        let height = world.height();
        let mut wanted: Vec<ChunkKey> = view
            .keys(LOAD_MARGIN)
            .filter(|key| height.is_none_or(|h| h.contains(&key.1)))
            .filter(|key| !world.chunks.contains_key(key) && !self.in_flight.contains(key))
            .collect();
        wanted.sort_unstable_by_key(|&key| Reverse(view.distance_sq(key)));

        self.in_flight.extend(wanted.iter().copied());
        self.shared.set_wanted(wanted);
    }
}

impl Drop for ChunkLoader {
    fn drop(&mut self) {
        self.shared.queue.lock().unwrap().shutdown = true;
        self.shared.wake.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn worker_loop(shared: &Shared, generate: &Generator, results: &Sender<(ChunkKey, Chunk)>) {
    loop {
        let key = {
            let mut queue = shared.queue.lock().unwrap();
            loop {
                if queue.shutdown {
                    return;
                }
                if let Some(key) = queue.wanted.pop() {
                    break key;
                }
                queue = shared.wake.wait(queue).unwrap();
            }
        };
        if results.send((key, generate(key))).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::player::settings::RenderDistance;
    use crate::game::world_gen::voxel::Voxel;
    use macroquad::math::vec3;
    use std::{
        collections::HashMap,
        time::{Duration, Instant},
    };

    const W: f32 = crate::constant::CHUNK_WIDTH as f32;
    const HEIGHT: std::ops::RangeInclusive<i32> = -1..=1;

    fn view_at(chunk: (i32, i32, i32), radius: usize) -> View {
        let position = vec3(chunk.0 as f32 * W, chunk.1 as f32 * W, chunk.2 as f32 * W);
        View::new(position, &RenderDistance { x: radius, y: radius, z: radius })
    }

    fn solid_chunk(_: ChunkKey) -> Chunk {
        let mut c = Chunk::new();
        c.set_voxel(0, 0, 0, Voxel::DIRT);
        c
    }

    /// Runs frames until every request has been answered.
    fn settle(loader: &mut ChunkLoader, world: &mut World, view: &View) {
        let start = Instant::now();
        loader.update(world, view, usize::MAX);
        while loader.in_flight() > 0 {
            assert!(start.elapsed() < Duration::from_secs(10), "loader never settled");
            thread::sleep(Duration::from_millis(1));
            loader.update(world, view, usize::MAX);
        }
    }

    fn expected_chunks(view: &View) -> usize {
        view.keys(LOAD_MARGIN).filter(|k| HEIGHT.contains(&k.1)).count()
    }

    #[test]
    fn loads_exactly_the_chunks_around_the_viewer() {
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(2, solid_chunk);
        let view = view_at((0, 0, 0), 3);
        settle(&mut loader, &mut world, &view);

        assert_eq!(world.chunks.len(), expected_chunks(&view));
        assert_eq!(world.chunks.len(), 9 * 9 * 3); // radius 3 + margin 1, three layers
        assert!(world.chunks.keys().all(|k| HEIGHT.contains(&k.1)));
    }

    #[test]
    fn nearest_chunks_are_generated_first() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let log = order.clone();
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(1, move |key| {
            log.lock().unwrap().push(key);
            solid_chunk(key)
        });
        let view = view_at((2, 0, -3), 3);
        settle(&mut loader, &mut world, &view);

        let order = order.lock().unwrap();
        assert_eq!(order[0], view.center());
        let distances: Vec<_> = order.iter().map(|&k| view.distance_sq(k)).collect();
        assert!(distances.windows(2).all(|w| w[0] <= w[1]), "generation order is not nearest-first");
    }

    #[test]
    fn works_with_many_workers() {
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(4, solid_chunk);
        let view = view_at((5, 0, -7), 4);
        settle(&mut loader, &mut world, &view);
        assert_eq!(world.chunks.len(), expected_chunks(&view));
    }

    #[test]
    fn nothing_is_requested_while_standing_still() {
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(2, solid_chunk);
        let view = view_at((0, 0, 0), 2);
        settle(&mut loader, &mut world, &view);
        loader.update(&mut world, &view, usize::MAX);
        assert_eq!(loader.in_flight(), 0);
    }

    #[test]
    fn moving_loads_new_chunks_and_unloads_far_ones() {
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(2, solid_chunk);
        settle(&mut loader, &mut world, &view_at((0, 0, 0), 2));

        let far = view_at((40, 0, 0), 2);
        settle(&mut loader, &mut world, &far);

        assert_eq!(world.chunks.len(), expected_chunks(&far));
        assert!(world.chunks.keys().all(|k| far.within(*k, LOAD_MARGIN)), "old chunks were not unloaded");
    }

    #[test]
    fn walking_a_long_way_keeps_memory_bounded() {
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(3, solid_chunk);
        let mut biggest = 0;
        for step in 0..60 {
            let view = view_at((step, 0, step / 2), 2);
            settle(&mut loader, &mut world, &view);
            biggest = biggest.max(world.chunks.len());
        }
        let keep_box = (2 * (2 + KEEP_MARGIN as usize) + 1).pow(2) * 3;
        assert!(biggest <= keep_box, "{biggest} chunks loaded, more than the {keep_box} we keep");
    }

    #[test]
    fn small_moves_never_generate_a_chunk_twice() {
        let generated = Arc::new(Mutex::new(Vec::new()));
        let log = generated.clone();
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(2, move |key| {
            log.lock().unwrap().push(key);
            solid_chunk(key)
        });
        for center in [(0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1), (0, 0, 0)] {
            settle(&mut loader, &mut world, &view_at(center, 3));
        }
        let keys = generated.lock().unwrap();
        let unique: HashSet<_> = keys.iter().collect();
        assert_eq!(keys.len(), unique.len(), "some chunks were generated more than once");
    }

    #[test]
    fn chunks_being_generated_when_the_player_crosses_a_border_are_not_requested_again() {
        let generated = Arc::new(Mutex::new(Vec::new()));
        let log = generated.clone();
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(1, move |key| {
            log.lock().unwrap().push(key);
            thread::sleep(Duration::from_millis(30));
            solid_chunk(key)
        });
        loader.update(&mut world, &view_at((0, 0, 0), 2), 0);
        while generated.lock().unwrap().is_empty() {
            thread::sleep(Duration::from_millis(1)); // wait until the worker is mid-chunk
        }
        settle(&mut loader, &mut world, &view_at((1, 0, 0), 2));

        let keys = generated.lock().unwrap();
        let unique: HashSet<_> = keys.iter().collect();
        assert_eq!(keys.len(), unique.len(), "a chunk was generated twice");
    }

    #[test]
    fn requests_for_places_the_player_left_are_cancelled() {
        let generated = Arc::new(Mutex::new(HashSet::new()));
        let log = generated.clone();
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(1, move |key| {
            thread::sleep(Duration::from_millis(2));
            log.lock().unwrap().insert(key);
            solid_chunk(key)
        });
        let first = view_at((0, 0, 0), 5);
        loader.update(&mut world, &first, 0); // queue ~363 chunks, about 0.7 s of work
        let second = view_at((200, 0, 0), 1);
        settle(&mut loader, &mut world, &second);

        let near_first = generated.lock().unwrap().iter().filter(|k| first.within(**k, LOAD_MARGIN)).count();
        assert!(near_first < 60, "{near_first} chunks of the abandoned area were still generated");
        assert_eq!(world.chunks.len(), expected_chunks(&second));
    }

    #[test]
    fn chunks_finishing_after_the_player_moved_on_are_dropped() {
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(1, |key| {
            thread::sleep(Duration::from_millis(30));
            solid_chunk(key)
        });
        loader.update(&mut world, &view_at((0, 0, 0), 0), 0);
        thread::sleep(Duration::from_millis(5)); // the worker is mid-way through its first chunk
        let elsewhere = view_at((500, 0, 0), 0);
        settle(&mut loader, &mut world, &elsewhere);
        assert!(world.chunks.keys().all(|k| elsewhere.within(*k, KEEP_MARGIN)));
    }

    #[test]
    fn everything_in_view_ends_up_meshed() {
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(3, solid_chunk);
        let mut view = view_at((0, 0, 0), 3);
        for step in 0..8 {
            view = view_at((step * 2, 0, -step), 3);
            settle(&mut loader, &mut world, &view);
        }
        world.rebuild_dirty(&view, usize::MAX);
        let starved: Vec<_> = world.chunks.iter().filter(|(k, c)| view.within(**k, 0) && c.is_dirty()).map(|(k, _)| *k).collect();
        assert!(starved.is_empty(), "never meshed: {starved:?}");
    }

    #[test]
    fn world_is_deterministic_regardless_of_thread_timing() {
        let make = |workers| {
            let mut world = World::streaming(HEIGHT);
            let mut loader = ChunkLoader::new(workers, |(x, y, z)| {
                let mut c = Chunk::new();
                c.set_voxel((x.rem_euclid(32)) as usize, (y.rem_euclid(32)) as usize, (z.rem_euclid(32)) as usize, Voxel::STONE);
                c
            });
            settle(&mut loader, &mut world, &view_at((3, 0, 3), 3));
            world.chunks.iter().map(|(k, c)| (*k, c.voxels().to_vec())).collect::<HashMap<_, _>>()
        };
        assert!(make(1) == make(4));
    }

    #[test]
    fn dropping_the_loader_stops_its_threads() {
        let mut world = World::streaming(HEIGHT);
        let mut loader = ChunkLoader::new(4, |key| {
            thread::sleep(Duration::from_millis(5));
            solid_chunk(key)
        });
        loader.update(&mut world, &view_at((0, 0, 0), 6), 0);
        let started = Instant::now();
        drop(loader); // joins the workers
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn default_workers_is_sensible() {
        assert!((1..=4).contains(&default_workers()));
    }
}