use simulation::{
    Position,
    field::{Diffuser, Neighbourhood, ResourceField},
};

fn main() -> Result<(), String> {
    let mut diffuser = Diffuser::new(Neighbourhood::VonNeumann);
    let mut resources = ResourceField::new_random(10, 10, 42, 1.2, 100.0);

    for _ in 0..10 {
        for y in 0..resources.height() {
            for x in 0..resources.width() {
                let position = Position::new(x, y);
                let resource_value = resources.get_resource_at(position).unwrap_or(0.0);

                print!("{:6.2} ", resource_value);
            }

            println!();
        }

        println!("\n\n");

        diffuser.diffuse(&mut resources, 0.01, 0.1, 1.0);
    }

    Ok(())
}
