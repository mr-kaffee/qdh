use clap::Parser;
use itertools::Itertools;
use minidom::{Element, NSChoice};
use nav_types::WGS84;
use std::{error::Error, fs};

#[derive(Parser)]
struct Cli {
    #[clap(parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    let root: Element = fs::read_to_string(args.path)?.parse()?;

    for (track_no, track) in root
        .children()
        .filter(|c| c.is("trk", NSChoice::Any))
        .enumerate()
    {
        let track_name = track
            .get_child("name", NSChoice::Any)
            .map(|e| e.text())
            .unwrap_or_else(|| "[unnamed]".to_string());

        for (segment_no, segment) in track
            .children()
            .filter(|c| c.is("trkseg", NSChoice::Any))
            .enumerate()
        {
            let points = segment
                .children()
                .filter(|c| c.is("trkpt", NSChoice::Any))
                .map(|e| element_to_coord(e))
                .collect::<Result<Vec<_>, _>>()?;

            let points = points
                .iter()
                .group_by(|p| (p.latitude_degrees(), p.longitude_degrees()))
                .into_iter()
                .map(|((latitude, longitude), g)| {
                    let (sum, cnt) =
                        g.fold((0.0, 0.0), |(sum, cnt), e| (sum + e.altitude(), cnt + 1.0));
                    WGS84::from_degrees_and_meters(latitude, longitude, sum / cnt)
                })
                .collect::<Vec<_>>();

            let (l, a, qdh) = points
                .iter()
                .zip(points.iter().skip(1))
                .map(|(a, b)| distances(a, b))
                .map(|(d, a, _, z)| {
                    if a <= 0.0 {
                        (d, 0.0, 0.0)
                    } else {
                        (d, z, d / 1000.0 * a * a)
                    }
                })
                .fold((0.0, 0.0, 0.0), |(sd, sa, sh), (d, a, h)| {
                    (sd + d, sa + a, sh + h)
                });

            println!(
                "Track #{:} ({:}), segment #{:}: {:.3} (distance: {:.3}km, ascend: {:.0}m)",
                track_no + 1,
                track_name,
                segment_no + 1,
                qdh,
                l / 1000.0,
                a
            );
        }
    }

    Ok(())
}

fn element_to_coord(e: &Element) -> Result<WGS84<f64>, Box<dyn Error>> {
    let latitude = e.attr("lat").ok_or("No latitude")?.parse::<f64>()?;
    let longitude = e.attr("lon").ok_or("No longitude")?.parse::<f64>()?;
    let altitude = e
        .get_child("ele", NSChoice::Any)
        .map(|a| a.text())
        .unwrap_or_else(|| "0".to_string())
        .parse::<f64>()?;

    Ok(WGS84::from_degrees_and_meters(
        latitude, longitude, altitude,
    ))
}

fn distances(a: &WGS84<f64>, b: &WGS84<f64>) -> (f64, f64, f64, f64) {
    let l = a.distance(&b);
    let z = b.altitude() - a.altitude();
    let x = f64::sqrt(l * l - z * z);

    (l, z / x * 100.0, x, z)
}
