fn main() -> Result<(), Box<dyn std::error::Error>> {
    kinograph_hero::build_plan()?.write_or_print(std::env::args().nth(1))?;
    Ok(())
}
