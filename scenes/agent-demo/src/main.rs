fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan = agent_demo::build_plan();
    plan.validate()?;
    plan.write_or_print(std::env::args().nth(1))?;
    Ok(())
}
