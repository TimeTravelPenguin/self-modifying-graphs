use simulation::{Position, simulation::resources::ResourceField};

fn main() -> Result<(), String> {
    let resources = ResourceField::new_random(10, 10, 42, 1.2, 100.0);
    for y in 0..resources.height() {
        for x in 0..resources.width() {
            let position = Position::new(x, y);
            let resource_value = resources.get_resource_at(position).unwrap_or(0.0);

            print!("{:6.2} ", resource_value);
        }
        println!();
    }

    Ok(())
}
