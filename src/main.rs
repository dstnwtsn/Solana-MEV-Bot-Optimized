use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use anyhow::Result;
use futures::FutureExt;
use log::{error, info};
use tokio::task::JoinSet;
use MEV_Bot_Solana::arbitrage::strategies::{
    optimism_tx_strategy,
    run_arbitrage_strategy,
    sorted_interesting_path_strategy,
};
use MEV_Bot_Solana::common::database::insert_vec_swap_path_selected_collection;
use MEV_Bot_Solana::common::types::InputVec;
use MEV_Bot_Solana::markets::pools::load_all_pools;
use MEV_Bot_Solana::transactions::create_transaction::{
    create_ata_extendlut_transaction,
    ChainType,
    SendOrSimulate,
};
use MEV_Bot_Solana::{
    common::constants::Env,
    common::utils::{from_str, get_tokens_infos, setup_logger},
    transactions::create_transaction::create_and_send_swap_transaction,
};
use MEV_Bot_Solana::arbitrage::types::{
    SwapPathResult,
    SwapPathSelected,
    TokenInArb,
    TokenInfos,
    VecSwapPathSelected,
};
use rust_socketio::{asynchronous::{Client, ClientBuilder}, Payload};

#[tokio::main]
async fn main() -> Result<()> {
    // Options
    let simulation_amount = 3_500_000_000; // 3.5 SOL
    let massive_strategy = true;
    let best_strategy = true;
    let optimism_strategy = true;

    // Massive strategy options
    let fetch_new_pools = false;
    let restrict_sol_usdc = true;

    // Best strategy options
    let path_best_strategy = "best_paths_selected/ultra_strategies/0-SOL-SOLLY-1-SOL-SPIKE-2-SOL-AMC-GME.json".to_string();

    // Optimism tx path
    let optimism_path = "optimism_transactions/11-6-2024-SOL-SOLLY-SOL-0.json".to_string();

    let mut inputs_vec = vec![
        InputVec {
            tokens_to_arb: vec![
                TokenInArb {
                    address: "So11111111111111111111111111111111111111112".into(),
                    symbol: "SOL".into(),
                },
                TokenInArb {
                    address: "4Cnk9EPnW5ixfLZatCPJjDB1PUtcRpVVgTQukm9epump".into(),
                    symbol: "DADDY-ANSEM".into(),
                },
            ],
            include_1hop: true,
            include_2hop: true,
            numbers_of_best_paths: 4,
            get_fresh_pools_bool: false,
        },
        InputVec {
            tokens_to_arb: vec![
                TokenInArb {
                    address: "So11111111111111111111111111111111111111112".into(),
                    symbol: "SOL".into(),
                },
                TokenInArb {
                    address: "2J5uSgqgarWoh7QDBmHSDA3d7UbfBKDZsdy1ypTSpump".into(),
                    symbol: "DADDY-TATE".into(),
                },
            ],
            include_1hop: true,
            include_2hop: true,
            numbers_of_best_paths: 4,
            get_fresh_pools_bool: false,
        },
        InputVec {
            tokens_to_arb: vec![
                TokenInArb {
                    address: "So11111111111111111111111111111111111111112".into(),
                    symbol: "SOL".into(),
                },
                TokenInArb {
                    address: "BX9yEgW8WkoWV8SvqTMMCynkQWreRTJ9ZS81dRXYnnR9".into(),
                    symbol: "SPIKE".into(),
                },
            ],
            include_1hop: true,
            include_2hop: true,
            numbers_of_best_paths: 2,
            get_fresh_pools_bool: false,
        },
        InputVec {
            tokens_to_arb: vec![
                TokenInArb {
                    address: "So11111111111111111111111111111111111111112".into(),
                    symbol: "SOL".into(),
                },
                TokenInArb {
                    address: "9jaZhJM6nMHTo4hY9DGabQ1HNuUWhJtm7js1fmKMVpkN".into(),
                    symbol: "AMC".into(),
                },
                TokenInArb {
                    address: "8wXtPeU6557ETkp9WHFY1n1EcU6NxDvbAggHGsMYiHsB".into(),
                    symbol: "GME".into(),
                },
            ],
            include_1hop: true,
            include_2hop: true,
            numbers_of_best_paths: 4,
            get_fresh_pools_bool: false,
        },
    ];

    dotenv::dotenv().ok();
    setup_logger()?;

    info!("Starting MEV_Bot_Solana");
    info!("⚠️ New fresh pools fetched on METEORA and RAYDIUM are excluded because they often have low liquidity");
    info!("⚠️ Liquidity is fetched from API and may be outdated on Raydium Pool");

    let mut set: JoinSet<()> = JoinSet::new();
    let tokens_to_arb: Vec<_> = inputs_vec.clone().into_iter().flat_map(|input| input.tokens_to_arb).collect();

    info!("Open Socket.IO channel...");
    let env = Env::new();
    
    let callback = |payload: Payload, _: Client| {
        async move {
            match payload {
                Payload::Text(data) => println!("Received: {:?}", data),
                Payload::Binary(data) => println!("Received bytes: {:#?}", data),
            }
        }
        .boxed()
    };
    
    let socket = ClientBuilder::new("wss://lively-shy-smoke.solana-mainnet.quiknode.pro/xxx")
        .namespace("/")
        .on("connection", callback)
        .on("error", |err, _| async move { error!("Socket.IO error: {}", err) }.boxed())
        .on("orca_quote", callback)
        .on("orca_quote_res", callback)
        .connect()
        .await?;

    if massive_strategy {
        info!("🏊 Fetching pools...");
        let dexs = load_all_pools(fetch_new_pools).await;
        info!("🏊 Loaded {} dexs", dexs.len());
        
        info!("🪙 Tokens: {:?}", tokens_to_arb);
        info!("📈 Starting arbitrage...");
        let mut vec_best_paths = Vec::new();
        for input_iter in inputs_vec.clone() {
            let tokens_infos = get_tokens_infos(input_iter.tokens_to_arb.clone()).await;

            let result = run_arbitrage_strategy(
                simulation_amount,
                input_iter.get_fresh_pools_bool,
                restrict_sol_usdc,
                input_iter.include_1hop,
                input_iter.include_2hop,
                input_iter.numbers_of_best_paths,
                dexs.clone(),
                input_iter.tokens_to_arb.clone(),
                tokens_infos.clone(),
            )
            .await?;
            let (path_for_best_strategy, _) = result;
            vec_best_paths.push(path_for_best_strategy);
        }
        if inputs_vec.len() > 1 {
            let mut vec_to_ultra_strategy: Vec<_> = Vec::new();
            let mut ultra_strategy_name = String::new();
            for (index, path) in vec_best_paths.iter().enumerate() {
                let name_parts: Vec<_> = path.split('/').collect();
                let name: Vec<_> = name_parts[1].split('.').collect();
                ultra_strategy_name = if index == 0 {
                    format!("{}-{}", index, name[0])
                } else {
                    format!("{}-{}-{}", ultra_strategy_name, index, name[0])
                };

                let file = File::open(path)?;
                let paths_vec: VecSwapPathSelected = serde_json::from_reader(file)?;
                vec_to_ultra_strategy.extend(paths_vec.value);
            }
            let path = format!("best_paths_selected/ultra_strategies/{}.json", ultra_strategy_name);
            let file = File::create(&path).map_err(|e| error!("Failed to create file {}: {}", path, e))?;
            let mut writer = BufWriter::new(file);
        
            let content = VecSwapPathSelected { value: vec_to_ultra_strategy };
            serde_json::to_writer(&writer, &content)?;
            writer.flush()?;
            info!("Written to {}", path);

            insert_vec_swap_path_selected_collection("ultra_strategies", content)
                .await
                .map_err(|e| {
                    error!("Failed to insert to ultra_strategies: {}", e);
                    e
                })?;

            path_best_strategy = path;
        }

        if best_strategy {
            let tokens_infos = get_tokens_infos(tokens_to_arb.clone()).await;
            sorted_interesting_path_strategy(simulation_amount, path_best_strategy.clone(), tokens_to_arb.clone(), tokens_infos.clone())
                .await?;
        }
    }
    
    if best_strategy && !massive_strategy {
        let tokens_infos = get_tokens_infos(tokens_to_arb.clone()).await;
        sorted_interesting_path_strategy(simulation_amount, path_best_strategy.clone(), tokens_to_arb.clone(), tokens_infos.clone())
            .await?;
    }
    
    if optimism_strategy {
        optimism_tx_strategy(optimism_path)?;
    }
    
    while let Some(res) = set.join_next().await {
        info!("{:?}", res);
    }

    println!("End");
    Ok(())
}