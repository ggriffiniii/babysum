extern crate babystats;
extern crate chrono;

use std::error::Error;
use std::collections::BTreeMap;
use std::io;
use std::process;
use babystats::BabyManagerData;

#[derive(Debug)]
struct Sum {
    total_diapers: i32,
    poo_diapers: i32,
    bottle_oz: f32,
    bottle_sessions: i32,
    breast_duration: chrono::Duration,
    pumping_oz: f32,
    tummy_time_duration: chrono::Duration,
    max_sleep_duration: chrono::Duration,
    total_sleep_duration: chrono::Duration,
}

impl Sum {
    fn new() -> Self {
        Sum{
            total_diapers: 0,
            poo_diapers: 0,
            bottle_oz: 0.0,
            bottle_sessions: 0,
            breast_duration: chrono::Duration::seconds(0),
            pumping_oz: 0.0,
            tummy_time_duration: chrono::Duration::seconds(0),
            max_sleep_duration: chrono::Duration::seconds(0),
            total_sleep_duration: chrono::Duration::seconds(0),
        }
    }
}

struct FormattedDuration(chrono::Duration);

impl std::fmt::Display for FormattedDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut secs = self.0.num_seconds();
        let hours = secs / 3600;
        if hours > 0 {
            write!(f, "{}h", hours)?;
        }
        secs -= hours * 3600;
        let minutes = secs / 60;
        if minutes > 0 || hours > 0 {
            write!(f, "{}m", minutes)?;
        }
        secs -= minutes * 60;
        write!(f, "{}s", secs)?;
        Ok(())
    }
}

impl std::fmt::Display for Sum {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Total Diapers: {}\n", self.total_diapers)?;
        write!(f, "Poo Diapers: {}\n", self.poo_diapers)?;
        write!(f, "Bottle: {:.1} oz ({:.1} oz per session)\n", self.bottle_oz, self.bottle_oz / self.bottle_sessions as f32)?;
        write!(f, "Bottle Sessions: {}\n", self.bottle_sessions)?;
        write!(f, "Breast Feeding: {}\n", FormattedDuration(self.breast_duration))?;
        write!(f, "Pumping: {:.1} oz\n", self.pumping_oz)?;
        write!(f, "Tummy Time: {}\n", FormattedDuration(self.tummy_time_duration))?;
        write!(f, "Max Sleep: {}\n", FormattedDuration(self.max_sleep_duration))?;
        write!(f, "Total Sleep: {}\n", FormattedDuration(self.total_sleep_duration))?;
        Ok(())
    }
}

fn run() -> Result<(), Box<Error>> {
    let mut rdr = BabyManagerData::from_reader(io::stdin());
    let mut events: Vec<_> = rdr.into_iter().map(|r| r.unwrap()).collect();
    events.sort_by_key(|e| e.time());
    let mut m: BTreeMap<_, _> = BTreeMap::new();
    let mut prev_bottle: Option<chrono::DateTime<chrono::Local>> = None;
    for event in events {
        match event {
            babystats::Event::Diaper(ref ev) => {
                let s = m.entry(ev.time.date()).or_insert(Sum::new());
                s.total_diapers += 1;
                if ev.poo {
                    s.poo_diapers += 1;
                }
            },
            babystats::Event::Feeding(ref ev) => {
                let s = m.entry(ev.time().date()).or_insert(Sum::new());
                match *ev {
                    babystats::FeedingEvent::Bottle(ref bev) => {
                        s.bottle_sessions += 1;
                        if let Some(prev) = prev_bottle {
                            if prev.date() == bev.time.date() && bev.time.signed_duration_since(prev) > chrono::Duration::minutes(60) {
                                s.bottle_sessions -= 1;
                            }
                        }
                        s.bottle_oz += bev.ounces;
                        prev_bottle = Some(bev.time);
                    },
                    babystats::FeedingEvent::LeftBreast(ref bev) | babystats::FeedingEvent::RightBreast(ref bev) => {
                        s.breast_duration = s.breast_duration + bev.duration;
                    },
                };
            },
            babystats::Event::Pumping(ref ev) => {
                let s = m.entry(ev.start.date()).or_insert(Sum::new());
                s.pumping_oz += ev.oz();
            },
            babystats::Event::TummyTime(ref ev) => {
                let s = m.entry(ev.start.date()).or_insert(Sum::new());
                s.tummy_time_duration = s.tummy_time_duration + ev.duration;
            },
            babystats::Event::Sleep(ref ev) => {
                if let Some(end) = ev.end {
                    let s = m.entry(end.date()).or_insert(Sum::new());
                    if ev.duration > s.max_sleep_duration {
                        s.max_sleep_duration = ev.duration;
                    }
                    s.total_sleep_duration = s.total_sleep_duration + ev.duration;
                }
            },
            _ => {},
        };
    }
    let summaries: Vec<_> = m.iter().map(|x| x).collect();
    const WINDOW_DAYS: usize = 7;
    for window in summaries.windows(WINDOW_DAYS) {
        let sum = window.iter().fold(Sum::new(), |mut acc, &(_, x)| {
            acc.total_diapers += x.total_diapers;
            acc.poo_diapers += x.poo_diapers;
            acc.bottle_oz += x.bottle_oz;
            acc.bottle_sessions += x.bottle_sessions;
            acc.breast_duration = acc.breast_duration + x.breast_duration;
            acc.pumping_oz += x.pumping_oz;
            acc.tummy_time_duration = acc.tummy_time_duration + x.tummy_time_duration;
            acc.max_sleep_duration = acc.max_sleep_duration + x.max_sleep_duration;
            acc.total_sleep_duration = acc.total_sleep_duration + x.total_sleep_duration;
            acc
        });
        let mean_sum = Sum{
            total_diapers: sum.total_diapers / WINDOW_DAYS as i32,
            poo_diapers: sum.poo_diapers / WINDOW_DAYS as i32,
            bottle_oz: sum.bottle_oz / WINDOW_DAYS as f32,
            bottle_sessions: sum.bottle_sessions / WINDOW_DAYS as i32,
            breast_duration: chrono::Duration::seconds(sum.breast_duration.num_seconds() / WINDOW_DAYS as i64),
            pumping_oz: sum.pumping_oz / WINDOW_DAYS as f32,
            tummy_time_duration: chrono::Duration::seconds(sum.tummy_time_duration.num_seconds() / WINDOW_DAYS as i64),
            max_sleep_duration: chrono::Duration::seconds(sum.max_sleep_duration.num_seconds() / WINDOW_DAYS as i64),
            total_sleep_duration: chrono::Duration::seconds(sum.total_sleep_duration.num_seconds() / WINDOW_DAYS as i64),
        };
        if let Some(&(date, _)) = window.last() {
            println!("{:?}:\n{}", date, mean_sum);
        }
    }
    //for (date, summary) in summaries {
    //    println!("{:?}: {:?}", date, summary);
    //}
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", err);
        process::exit(1);
    }
}
