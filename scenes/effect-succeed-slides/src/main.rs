fn main() -> anyhow::Result<()> {
    kinograph_effect_succeed_slides::build_plan()?.write_or_print(std::env::args().nth(1))?;
    Ok(())
}
