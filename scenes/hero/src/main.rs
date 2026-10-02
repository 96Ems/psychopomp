fn main() -> Result<(), Box<dyn std::error::Error>> {
    psychopomp_hero::build_plan()?.write_or_print(std::env::args().nth(1))?;
    Ok(())
}
