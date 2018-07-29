extern crate chrono;
extern crate clap;
extern crate quick_xml;
// extern crate gps;

use clap::{App, Arg, SubCommand};

mod gps;

pub use gps::Point;
pub use gps::Track;

fn main() {
    let matches = App::new("Gpxtools")
        .arg(
            Arg::with_name("TRACK")
                .required(true)
                .help("GPX file to process")
                .index(1)
        )
        .arg(
            Arg::with_name("maxpoints")
                .long("maxpoints")
                .value_name("NUM")
                .help("Enter maximum points per track")
                .takes_value(true),
        )
        .get_matches();

    let trackfile = matches.value_of("TRACK").unwrap_or("data/Day1-1.gpx");
    let maxpts: usize = matches.value_of("maxpoints").unwrap_or("3000").parse().unwrap();

    let mut trk = Track {
        ..Default::default()
    };

    trk.from_xml(trackfile.to_string());
    let subtracks: usize = (trk.points.len() as f32 / maxpts as f32).round() as usize;

    println!("Will generate {:?} tracks", subtracks);

    println!("Track name:   {:?}", trk.name);

    for i in 0..subtracks {
        let mut segment = Track {
            name: trk.name.clone() + "_" + i.to_string().as_str(),
            ..Default::default()
        };
        let startpoint: usize = i*maxpts;
        let mut endpoint: usize = (i+1)*maxpts-1;

        //safeguard
        if trk.points.len()-1 < endpoint {
            endpoint = trk.points.len()-1;
        }

        segment.points = trk.points[startpoint .. endpoint].to_vec();
        println!("Seg dist:     {:?} km", segment.len());
        println!("Seg duration: {:?} h", segment.time().num_minutes() as f32 / 60.0);
        println!("Seg speed:    {:?} Km/h", segment.speed());
        println!("Seg pts:      {}-{}", startpoint, endpoint);
        segment.to_xml();

    }


}
