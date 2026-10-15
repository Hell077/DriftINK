use crate::ipc::Output;
/// World centers are Y-up; surface coordinates are logical pixels, Y-down.
pub fn world_to_screen(world: [f64; 2], output: &Output) -> [f64; 2] {
    [
        f64::from(output.size[0]) / 2.0 + (world[0] - output.camera[0]) * output.zoom,
        f64::from(output.size[1]) / 2.0 - (world[1] - output.camera[1]) * output.zoom,
    ]
}
/// Inverse projection from output-local logical pixels into the shared Y-up world.
pub fn screen_to_world(screen: [f64; 2], output: &Output) -> [f64; 2] {
    [
        output.camera[0] + (screen[0] - f64::from(output.size[0]) / 2.0) / output.zoom,
        output.camera[1] - (screen[1] - f64::from(output.size[1]) / 2.0) / output.zoom,
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn centers_and_y_axis_at_different_zooms() {
        for zoom in [0.25, 1.0, 2.5] {
            let o = Output {
                name: "test".into(),
                active: false,
                camera: [-100.0, 200.0],
                zoom,
                size: [1920, 1080],
            };
            assert_eq!(world_to_screen(o.camera, &o), [960.0, 540.0]);
            assert_eq!(
                world_to_screen([-90.0, 220.0], &o),
                [960.0 + 10.0 * zoom, 540.0 - 20.0 * zoom]
            );
        }
    }
}

#[cfg(test)]
mod inverse_tests {
    use super::*;
    #[test]
    fn round_trip_across_outputs_and_large_coordinates() {
        for size in [[1920, 1080], [2560, 1440], [1536, 864]] {
            for zoom in [0.125, 1.0, 1.25, 4.0] {
                let o = Output {
                    name: "test".into(),
                    active: false,
                    camera: [1e8, -1e8],
                    zoom,
                    size,
                };
                for p in [[1e8 + 100.0, -1e8 - 250.0], [-100.0, 200.0]] {
                    let restored = screen_to_world(world_to_screen(p, &o), &o);
                    assert!((restored[0] - p[0]).abs() < 1e-6 && (restored[1] - p[1]).abs() < 1e-6);
                }
            }
        }
    }
}
