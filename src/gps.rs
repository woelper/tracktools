#![feature(extern_prelude)]
extern crate chrono;
extern crate quick_xml;

use chrono::Duration;
use chrono::TimeZone;
use chrono::Utc;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use std::fs::File;
use std::io::prelude::*;
use std::io::Cursor;
use std::path::Path;
use std::time::Duration as stdDuration;

#[derive(Debug, Clone)]
pub struct Point {
    pub lat: f64,
    pub long: f64,
    pub ele: f64,
    pub time: chrono::DateTime<Utc>,
}

impl Point {
    pub fn new() -> Point {
        Point {
            lat: 0.0,
            long: 0.0,
            ele: 0.0,
            time: Utc.timestamp(1, 1),
        }
    }
}

#[derive(Default, Debug)]
pub struct Track {
    pub name: String,
    pub points: Vec<Point>,
}

impl Track {
    pub fn len(&self) -> f64 {
        let mut sum = 0.0;
        let mut prev_point = &self.points[0];

        for pt in &self.points {
            // println!("{:?}  {:?}", prev_point, pt);
            // println!("{:?}", pt);
            let d = dist(prev_point, pt);
            sum += d;
            prev_point = pt;
        }
        sum
    }

    pub fn to_xml(&self, autoseg: bool) {
        let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), 32, 4); //32 is space character

        let mut gpx_elem = BytesStart::owned(b"gpx".to_vec(), "gpx".len());
        gpx_elem.push_attribute(("creator", "Pothole"));
        assert!(writer.write_event(Event::Start(gpx_elem)).is_ok());

        let trk_elem = BytesStart::owned(b"trk".to_vec(), "trk".len());
        assert!(writer.write_event(Event::Start(trk_elem)).is_ok());

        let mut prev_point = &self.points[0];

        let trkseg_elem = BytesStart::owned(b"trkseg".to_vec(), "trkseg".len());
        assert!(writer.write_event(Event::Start(trkseg_elem.clone())).is_ok());

        let mut ptnum = 0;
        for pt in &self.points {
            //segment detection
            let d = dist(prev_point, pt);
            prev_point = pt;
            // if there is a gap to the last point, consider this a new segment
            if d > 0.5 {
                assert!(
                    writer
                        .write_event(Event::End(BytesEnd::borrowed(b"trkseg")))
                        .is_ok()
                );
                // let trkseg_elem = BytesStart::owned(b"trkseg".to_vec(), "trkseg".len());
                assert!(writer.write_event(Event::Start(trkseg_elem.clone())).is_ok());
            }



            let mut pt_elem = BytesStart::owned(b"trkpt".to_vec(), "trkpt".len());
            pt_elem.push_attribute(("lat", pt.lat.to_string().as_str()));
            pt_elem.push_attribute(("lon", pt.long.to_string().as_str()));
            pt_elem.push_attribute(("ele", pt.ele.to_string().as_str()));
            assert!(writer.write_event(Event::Start(pt_elem)).is_ok());
            assert!(
                writer
                    .write_event(Event::End(BytesEnd::borrowed(b"trkpt")))
                    .is_ok()
            );
            // println!("{} {}", ptnum, self.points.len());
            if ptnum >= self.points.len()-1 {
                assert!(
                    writer
                        .write_event(Event::End(BytesEnd::borrowed(b"trkseg")))
                        .is_ok()
                );
            }
            ptnum+=1;
        }

        assert!(
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"trk")))
                .is_ok()
        );
        assert!(
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"gpx")))
                .is_ok()
        );

        let result = writer.into_inner().into_inner();
        // println!("{:?}", str::from_utf8( &result).unwrap());

        let mut name: String = self.name.clone();
        name.push_str(".gpx");
        write_bytes(result, name);
    }

    pub fn time(&self) -> chrono::Duration {
        // let timespan = &self.points[0].time.signed_duration_since(self.points[1].time);
        let mut sum = Duration::from_std(stdDuration::new(0, 0)).unwrap();
        let mut prev_time = &self.points[0].time;

        for pt in &self.points {
            let timespan = prev_time.signed_duration_since(pt.time);
            sum = sum.checked_sub(&timespan).unwrap();
            prev_time = &pt.time;
        }
        sum
    }

    pub fn speed(&self) -> f64 {
        self.len() / (self.time().num_seconds() as f64 / 3600.0)
    }

    fn truncate_by_length(&mut self, length_km: f64) {
        while self.len() > length_km {
            self.points.remove(0);
        }
    }

    pub fn parse(&mut self) {
        // how far to average the track together
        let sample_distance = 0.1;
        // This is where a track is considered bad = min_speed_factor * your average over sample_distance
        let min_speed_factor = 0.6;

        let global_average_speed = self.speed();

        let mut fifo_track = Track {
            ..Default::default()
        };

        let mut analyzed_track = Track {
            name: "analyzed".to_string(),
            ..Default::default()
        };

        let mut bad_track = Track {
            name: "bad".to_string(),
            ..Default::default()
        };

        for pt in &self.points {
            // i = i + 1; if i > 30 {break};
            fifo_track.points.push(pt.clone());
            analyzed_track.points.push(pt.clone());
            fifo_track.truncate_by_length(sample_distance);
            let speed = fifo_track.speed();
            if speed < global_average_speed * min_speed_factor {
                bad_track.points.push(pt.clone());
                // println!("Track seems bad @ Km {:.1} {:.2} Km/h", analyzed_track.len(), speed);
            }
        }
        bad_track.to_xml(false);
    }

    pub fn from_xml(&mut self, filename: String) {
        // Records for the pull parser
        let mut current_point = Point::new();
        let mut current_data = "".to_string();

        // println!("XML parse start.");

        let mut reader = Reader::from_file(filename).unwrap();
        reader.trim_text(true);
        let mut buf = Vec::new();

        // The `Reader` does not implement `Iterator` because it outputs borrowed data (`Cow`s)
        loop {
            match reader.read_event(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    // println!("{:?}", e.unescape_and_decode(&reader).unwrap());

                    match e.name() {
                        b"trkpt" => {
                            for a in e.attributes() {
                                let unwrapped_attr = a.unwrap();
                                let key = reader.decode(unwrapped_attr.key).into_owned();
                                let value =
                                    unwrapped_attr.unescape_and_decode_value(&reader).unwrap();

                                match key.as_str() {
                                    "lat" => current_point.lat = value.parse().unwrap(),
                                    "lon" => current_point.long = value.parse().unwrap(),
                                    "ele" => current_point.ele = value.parse().unwrap(),
                                    _ => (),
                                }
                            }
                            ()
                        }
                        _ => (),
                    }
                }
                // current_data = text;
                Ok(Event::Text(e)) => {
                    current_data = e.unescape_and_decode(&reader).unwrap();
                    // txt.push(e.unescape_and_decode(&reader).unwrap()),
                }
                Ok(Event::End(ref e)) => {
                    match e.name() {
                        b"ele" => {
                            current_point.ele = current_data.parse().unwrap();
                        }
                        b"name" => {
                            self.name = current_data.clone();
                        }
                        b"time" => {
                            match Utc.datetime_from_str(current_data.as_str(), "%FT%H:%M:%S%.3fZ") {
                                Ok(time) => current_point.time = time,
                                Err(err) => println!("=== TIME ERROR === {:?}", err),
                            }
                        }
                        b"trkpt" => {
                            // push a copy of the point to the track
                            self.points.push(current_point.clone());
                        }
                        _ => (),
                    }
                }
                Ok(Event::Eof) => break, // exits the loop when reaching end of file
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => (), // There are several other `Event`s we do not consider here
            }

            // if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
            buf.clear();
        }

        // println!("XML parse end.");
    }
}

fn dist(p1: &Point, p2: &Point) -> f64 {
    let r = 6371.0;
    let d_lat = (p1.lat - p2.lat).to_radians();
    let d_long = (p1.long - p2.long).to_radians();
    let a = (d_lat / 2.0).sin() * (d_lat / 2.0).sin()
        + p1.lat.to_radians().cos()
            * p2.lat.to_radians().cos()
            * (d_long / 2.0).sin()
            * (d_long / 2.0).sin();
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    return r * c;
}

fn write_bytes(bytes_to_write: Vec<u8>, filename: String) {
    let path = Path::new(filename.as_str());
    let display = path.display();

    // Open a file in write-only mode, returns `io::Result<File>`
    let mut file = match File::create(&path) {
        Err(_why) => panic!("couldn't create {}", display),
        Ok(file) => file,
    };

    // Write the `LOREM_IPSUM` string to `file`, returns `io::Result<()>`
    match file.write_all("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n".as_bytes()) {
        Err(_why) => panic!("couldn't write to {}", display),
        // Ok(_) => (println!("Wrote header -> {}", display)),
        Ok(_) => (),
    }

    match file.write_all(&bytes_to_write) {
        Err(_why) => panic!("couldn't write to {}", display),
        Ok(_) => println!("Wrote xml:    {}", display),
        Ok(_) => println!(),
    }
}
