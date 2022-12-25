extern crate market_maker;

use rust_decimal_macros::dec;

use market_maker::apikey::ApiKey;
use market_maker::bot::{Bot, Config};
use market_maker::implements::exchanges::bitmex::{BitMEXBroker, BitMEXMarket, BitMEXStatus};
use market_maker::logger;
use market_maker::strategies::dbo::DepthBasedOffering;

fn main() {
    logger::setup_with("info");

    // exchange
    let apikey = ApiKey::read_json("./keys/bitmex.json").expect("apikey not found");
    let market = BitMEXMarket::connect();
    let status = BitMEXStatus::connect(&apikey);
    let broker = BitMEXBroker::connect(&apikey);

    // strategy
    let policy = DepthBasedOffering::new(dec!(200), dec!(1000));

    // bot
    let config = Config {
        num_iteration: usize::MAX,
        test: true,
    };
    let mut bot = Bot::new(config, market, status, broker, policy);
    bot.run().unwrap();
}
