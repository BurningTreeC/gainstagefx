//! Reduced exterior diffraction around a rectangular enclosure.
//!
//! Direct radiation and surface paths to all visible edges are summed, with
//! individual lengths and arrival directions. The edge graph works on every
//! face, including receivers beside or behind the cabinet. It approximates a
//! finite-baffle boundary integral; its weights are not a measured scattering
//! solution. No single "nearest edge" is selected when the receiver moves.

use super::cabinet::CabinetProfile;
use super::enclosure::RADIATORS;

pub const EDGES: usize = 12;
pub const ROUTES: usize = EDGES + 1;

#[derive(Clone, Copy, Debug, Default)]
pub struct Path {
    pub distance: f64,
    /// Direction from receiver toward the final radiating point.
    pub arrival: [f64; 3],
    pub weight: f64,
    pub around: f64,
}

#[derive(Clone, Copy)]
struct Edge {
    point: [f64; 3],
    faces: [usize; 2],
}

const NORMALS: [[f64; 3]; 6] = [
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
    [-1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 1.0, 0.0],
];

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn delta(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// Sources: opening, front/back, left/right, bottom/top. Coordinates put the
/// front baffle at z=0 and the rear at z=-depth. The opening is the back's
/// centre for an open-backed cabinet and the ports' position on the baffle for
/// a vented one.
pub fn paths(cab: &CabinetProfile, source: usize, receiver: [f64; 3]) -> [Path; ROUTES] {
    assert!(source < RADIATORS);
    let (w, h, d) = (cab.width / 2.0, cab.height / 2.0, cab.depth);
    let faces = [
        [0.0, 0.0, 0.0],
        [0.0, 0.0, -d],
        [-w, 0.0, -d / 2.0],
        [w, 0.0, -d / 2.0],
        [0.0, -h, -d / 2.0],
        [0.0, h, -d / 2.0],
    ];
    let (face, origin) = match (source, cab.vent) {
        (0, Some(vent)) => (0, [vent.position.0, vent.position.1, 0.0]),
        (0, None) => (1, faces[1]),
        (s, _) => (s - 1, faces[s - 1]),
    };
    let edges = [
        Edge {
            point: [-w, 0.0, 0.0],
            faces: [0, 2],
        },
        Edge {
            point: [w, 0.0, 0.0],
            faces: [0, 3],
        },
        Edge {
            point: [0.0, -h, 0.0],
            faces: [0, 4],
        },
        Edge {
            point: [0.0, h, 0.0],
            faces: [0, 5],
        },
        Edge {
            point: [-w, 0.0, -d],
            faces: [1, 2],
        },
        Edge {
            point: [w, 0.0, -d],
            faces: [1, 3],
        },
        Edge {
            point: [0.0, -h, -d],
            faces: [1, 4],
        },
        Edge {
            point: [0.0, h, -d],
            faces: [1, 5],
        },
        Edge {
            point: [-w, -h, -d / 2.0],
            faces: [2, 4],
        },
        Edge {
            point: [-w, h, -d / 2.0],
            faces: [2, 5],
        },
        Edge {
            point: [w, -h, -d / 2.0],
            faces: [3, 4],
        },
        Edge {
            point: [w, h, -d / 2.0],
            faces: [3, 5],
        },
    ];
    // Shortest routes along adjacent faces. Distances never cut through the
    // box. The mesh is deliberately small: this is evaluated at control rate.
    let mut distance = edges.map(|e| {
        if e.faces.contains(&face) {
            length(delta(e.point, origin))
        } else {
            f64::INFINITY
        }
    });
    let mut visited = [false; EDGES];
    for _ in 0..EDGES {
        let i = (0..EDGES)
            .filter(|&i| !visited[i])
            .min_by(|&i, &j| distance[i].total_cmp(&distance[j]))
            .unwrap();
        visited[i] = true;
        for j in 0..EDGES {
            if edges[i].faces.iter().any(|f| edges[j].faces.contains(f)) {
                distance[j] =
                    distance[j].min(distance[i] + length(delta(edges[j].point, edges[i].point)));
            }
        }
    }
    let incoming = delta(origin, receiver);
    let direct_length = length(incoming).max(1e-5);
    let facing = -dot(incoming, NORMALS[face]) / direct_length;
    let direct = (2.0 * facing).clamp(0.0, 1.0);
    let mut result = [Path::default(); ROUTES];
    result[0] = Path {
        distance: direct_length,
        arrival: incoming.map(|v| v / direct_length),
        weight: direct,
        around: 0.0,
    };
    let mut total = 0.0;
    for (i, e) in edges.iter().enumerate() {
        let incoming = delta(e.point, receiver);
        let front = length(incoming).max(1e-5);
        // Projected visible-face weight goes continuously to zero at a face's
        // horizon; oblique receivers hear multiple adjoining sets of edges.
        let weight = e
            .faces
            .iter()
            .map(|&f| (-dot(incoming, NORMALS[f]) / front).max(0.0))
            .sum::<f64>();
        total += weight;
        result[i + 1] = Path {
            distance: distance[i] + front,
            arrival: incoming.map(|v| v / front),
            weight,
            around: distance[i],
        };
    }
    for p in &mut result[1..] {
        p.weight *= (1.0 - direct) / total.max(1e-20);
    }
    result
}
