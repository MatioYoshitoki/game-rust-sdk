use match_kernel::{ConfigCatalog, JSONL_LINE_LIMIT, Runtime, protocol::*, sdk_info};
use serde::Deserialize;
use std::io::{self, BufRead, Read, Write};

/// Typed responses keep the JSONL wire bytes identical to the historical
/// `json!`-wrapped shapes while streaming straight to the writer.
#[derive(serde::Serialize)]
#[serde(untagged)]
enum Response {
    Status { status: Status },
    Loadout { loadout: SkillLoadoutReceipt },
    Equipment { equipment: WeaponEquipmentReceipt },
    Outputs { outputs: Vec<Output> },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Create { request: CreateRequest },
    SubmitInput { request: Input },
    SetSkillLoadout { request: SetSkillLoadout },
    SetEquippedWeapon { request: SetEquippedWeapon },
    ResetTrainingCooldowns { request: ResetTrainingCooldowns },
    RestoreTrainingMember { request: RestoreTrainingMember },
    SpawnTrainingTargets { request: SpawnTrainingTargets },
    DespawnTrainingTargets { request: DespawnTrainingTargets },
    SubmitBusinessReceipt { request: BusinessReceiptRequest },
    SubmitTransportReceipt { request: TransportReceipt },
    Spectate { request: SpectateV6 },
    DrainOutputs {},
    Close { request: Close },
}

fn discard_line(input: &mut impl BufRead) -> io::Result<()> {
    loop {
        let bytes = input.fill_buf()?;
        if bytes.is_empty() {
            return Ok(());
        }
        let newline = bytes.iter().position(|byte| *byte == b'\n');
        let count = newline.map_or(bytes.len(), |index| index + 1);
        input.consume(count);
        if newline.is_some() {
            return Ok(());
        }
    }
}

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--sdk-info") {
        println!("{}", sdk_info());
        return Ok(());
    }
    let missing_catalog = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "supply a host configuration catalog via --configs <path> or MATCH_CONFIG_MANIFEST",
        )
    };
    let path = match args.iter().position(|arg| arg == "--configs") {
        Some(index) => args
            .get(index + 1)
            .filter(|path| !path.starts_with("--"))
            .cloned(),
        None => std::env::var("MATCH_CONFIG_MANIFEST").ok(),
    }
    .filter(|path| !path.is_empty())
    .ok_or_else(missing_catalog)?;
    let catalog = std::sync::Arc::new(ConfigCatalog::load(path)?);
    if args.iter().any(|arg| arg == "--catalog") {
        println!("{}", catalog.describe());
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--config-hash") {
        println!("{}", catalog.default_config().content_hash());
        return Ok(());
    }
    let verification = std::env::args().any(|arg| arg == "--verification");
    #[cfg(feature = "replay")]
    let runtime = if verification {
        Runtime::verification_with_catalog(catalog)
    } else {
        Runtime::with_catalog(catalog)
    };
    #[cfg(not(feature = "replay"))]
    let runtime = {
        if verification {
            eprintln!("verification clock requires --features replay");
            std::process::exit(2);
        }
        Runtime::with_catalog(catalog)
    };
    let client = runtime.client();
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut bytes = Vec::new();
        let count = (&mut input)
            .take((JSONL_LINE_LIMIT + 1) as u64)
            .read_until(b'\n', &mut bytes)?;
        if count == 0 {
            break;
        }
        let too_large = count > JSONL_LINE_LIMIT;
        if too_large && bytes.last() != Some(&b'\n') {
            discard_line(&mut input)?;
        }
        let result = if too_large {
            Err(Error::InvalidMessage)
        } else {
            serde_json::from_slice::<Command>(&bytes)
                .map_err(|_| Error::InvalidMessage)
                .and_then(|command| match command {
                    Command::Create { request } => client
                        .create(request)
                        .map(|status| Response::Status { status }),
                    Command::SetSkillLoadout { request } => client
                        .set_skill_loadout(request)
                        .map(|loadout| Response::Loadout { loadout }),
                    Command::SetEquippedWeapon { request } => client
                        .set_equipped_weapon(request)
                        .map(|equipment| Response::Equipment { equipment }),
                    Command::ResetTrainingCooldowns { request } => client
                        .reset_training_cooldowns(request)
                        .map(|status| Response::Status { status }),
                    Command::RestoreTrainingMember { request } => client
                        .restore_training_member(request)
                        .map(|status| Response::Status { status }),
                    Command::SpawnTrainingTargets { request } => client
                        .spawn_training_targets(request)
                        .map(|status| Response::Status { status }),
                    Command::DespawnTrainingTargets { request } => client
                        .despawn_training_targets(request)
                        .map(|status| Response::Status { status }),
                    Command::SubmitInput { request } => client
                        .submit_input(request)
                        .map(|status| Response::Status { status }),
                    Command::SubmitBusinessReceipt { request } => client
                        .submit_business_receipt(request)
                        .map(|status| Response::Status { status }),
                    Command::SubmitTransportReceipt { request } => client
                        .submit_transport_receipt(request)
                        .map(|status| Response::Status { status }),
                    Command::Spectate { request } => client
                        .spectate(request)
                        .map(|status| Response::Status { status }),
                    Command::Close { request } => client
                        .close(request)
                        .map(|status| Response::Status { status }),
                    Command::DrainOutputs {} => client
                        .drain_outputs()
                        .map(|outputs| Response::Outputs { outputs }),
                })
        };
        // Stream the envelope straight to the writer: building a
        // serde_json::Value tree first would copy every drained output.
        match result {
            Ok(value) => {
                write!(output, "{{\"ok\":")?;
                serde_json::to_writer(&mut output, &value)?;
                writeln!(output, "}}")?;
            }
            Err(error) => {
                write!(output, "{{\"error\":")?;
                serde_json::to_writer(&mut output, &error)?;
                writeln!(output, "}}")?;
            }
        }
        output.flush()?;
    }
    drop(runtime);
    Ok(())
}
