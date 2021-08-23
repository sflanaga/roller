use std::time::{Instant, Duration};
use core::cmp;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::borrow::Borrow;
use std::ops::Deref;
use chrono::{Local, DateTime};
use num_format::{Locale, ToFormattedString, ToFormattedStr};
use thread_priority::{set_current_thread_priority, ThreadPriority};

struct SimulationResults {
    index: usize,
    max_iterations: u64,
    iterations_done: u64,
    ability: [u64; 19],
    sum_abilities: [u64; 109],
    check_interval: u64,
    cnt_two_18s: u64,
}

impl SimulationResults {
    fn new(index: usize, iterations: u64) -> Self {
        SimulationResults {
            index,
            max_iterations: iterations,
            iterations_done: 0,
            ability: [0u64; 19],
            sum_abilities: [0u64; 109],
            check_interval: 1000,
            cnt_two_18s: 0,
        }
    }

    fn add_to(self: &mut Self, other: &Self) {
        self.iterations_done += other.iterations_done;
        ray_merge(&mut self.ability, &other.ability);
        ray_merge(&mut self.sum_abilities, &other.sum_abilities);
        self.cnt_two_18s += other.cnt_two_18s;
    }


}

fn ray_merge<T: std::ops::AddAssign + std::clone::Clone>(r1:&mut [T], r2: &[T]) {
    // for i in 0..r2.len() {
    //     r1[i] += r2[i].copy();
    // }
    //1
    for (i,v) in r2.iter().enumerate() {
        r1[i] += v.clone();
    }
}


fn run(index: usize, iterations: u64, stop: &mut Arc<AtomicBool>, count: &mut Arc<AtomicU64>) -> SimulationResults {
    set_current_thread_priority(ThreadPriority::Min).expect("Cannot lower thread priority to minimum");
    let mut sim = SimulationResults::new(index, iterations);
    let rng = fastrand::Rng::new();
    rng.seed(index as u64);
    let start_time = Instant::now();
    let mut update_stats = sim.check_interval;
    for i in 0..sim.max_iterations {
        update_stats -= 1;
        let mut abilities = [0u8; 6];
        let mut ability_sum = 0u8;
        let mut cnt_18s = 0;
        for a in 0..6 {
            let mut ability = 0u8;
            let mut min = 10;
            // println!();
            for r in 0..4 {
                let roll = rng.u8(1..7); // ran.gen_range(1..7);
                ability += roll;
                min = cmp::min(min, roll);
                // println!("roll: {}", roll);
            }
            ability -= min;
            if ability >= 18 { cnt_18s += 1; }
            // println!("ability: {}", ability);

            ability_sum += ability;
            abilities[a] = ability;
            // println!("ability: {}", ability);
            sim.ability[ability as usize] += 1;
        }
        if cnt_18s >= 2 {
            sim.cnt_two_18s += 1;
            //self.print_ray("\nI have a larry: ", &abilities);
        }
        sim.sum_abilities[ability_sum as usize] += 1;
        if update_stats <= 0 {
            sim.iterations_done = i;
            count.fetch_add(sim.check_interval, Ordering::Relaxed);
            update_stats = sim.check_interval;
            if stop.load(Ordering::Relaxed) {
                println!("thread {} stopping at iteration {}", index, i);
                break;
            }
        }
    }

    return sim;
}

fn print_results(sim: &SimulationResults) {
    print_ray("\n\nability counts: ", &sim.ability);
    print_ray("ability sums: ", &sim.sum_abilities);
    println!("number of 2 18s or more: {}", sim.cnt_two_18s);
    println!("total characters made: {}", &sim.iterations_done.to_formatted_string(&Locale::en));
}

fn print_ray<T: std::fmt::Display + ToFormattedStr>(msg: &str, ray: &[T]) {
    println!("\n{}", msg);
    for (i, v) in ray.iter().enumerate() {
        println!("{} = {}", i, v.to_formatted_string(&Locale::en));
    }
}


fn main() {
    let cpus = num_cpus::get();
    let iters = 1_000_000;
    let no_threads = cpus;
    println!("running {} threads - one per cpu, but at low priority", no_threads);
    let mut stop = Arc::new(AtomicBool::new(false));
    let mut count_so_far =  Arc::new(AtomicU64::new(0));

    let start_time = Instant::now();
    let mut th = vec![];
    for i in 0..no_threads {
        let mut stop_clone = stop.clone();
        let mut count_so_far_clone = count_so_far.clone();
        th.push(std::thread::spawn(move || run(i, iters, &mut stop_clone, &mut count_so_far_clone) ) );
    }
    { // stop on Ctrl-C thread
        let mut stop_handler = stop.clone();
        let start_dt = Local::now();
        ctrlc::set_handler(move || {
            let dur = Local::now() - start_dt;
            println!("\n\n Exiting at {} ran for {:}", &now_str(), humantime::format_duration(dur.to_std().unwrap()).to_string());
            stop_handler.store(true, Ordering::Relaxed);
            //std::process::exit(0);
        }).expect("Error setting Ctrl-C handler");
    }
    { // ticker thread
        let count_clone = count_so_far.clone();
        std::thread::spawn(move||{
            let mut last_count = 0;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let cnt = count_clone.load(Ordering::Relaxed);
                let diff = cnt - last_count;
                last_count = cnt;
                println!("characters per second: {}    Total so far: {}",
                         diff.to_formatted_string(&Locale::en),
                         cnt.to_formatted_string(&Locale::en));
            }
        });
    }
    println!("waiting for results");

    let mut sim = SimulationResults::new(0, 0);
    for t in th {
        let x = t.join();
        match x {
            Ok(sim_r) => { sim.add_to(&sim_r); },
            Err(e) => println!("error joining {:?}", &e),
        }

    }
    //let sim = run(0, iters, &mut stop);
    print_results(&sim);
    let e = start_time.elapsed();
    println!("done in {:?}  rate: {}", e, (((sim.iterations_done as f64 * 1000.0) / (e.as_millis() as f64)) as u64).to_formatted_string(&Locale::en));
}

fn now_str() -> String {
    let dt: DateTime<Local> = Local::now();
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}
