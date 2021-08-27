use solana_program::{
    pubkey::Pubkey,
};

pub struct Oracle {
    // Period for moving aveage
    pub PERIOD: u32,

    // Program id for token0
    pub token0: &Pubkey,

    // Program id for token1
    pub token1: &Pubkey,

    // cumulative price for token0
    pub price0CumulativeLast: u32,

    // cumulative price for token1
    pub price1CumulativeLast: u32,

    // last block timestamp
    pub blockTimestampLast: u32,

    // average price for token0
    price0Average: f64,

    // average price for token1
    price1Average: f64,
}

impl Oracle {
    pub fn new(
        token0: &Pubkey,
        token1: &Pubkey,
    ) -> Self {
        Self {
            PERIOD: 0,
            token0: Pubkey::from(token0),
            token1: Pubkey::from(token1),
            price0CumulativeLast: 0,
            price1CumulativeLast: 0,
            blockTimestampLast: 0,
            price0Average: 0.0,
            price1Average: 0.0
        }
    }

    pub fn update(
        &self,
        price0Cumulative: f64,
        price1Cumulative: f64,
        blockTimestamp: u32
    ) {

        let timeElapsed: u32 = blockTimestamp - self.blockTimestampLast?; // overflow is desired

        // ensure that at least one full period has passed since the last update
        if timeElapsed >= self.PERIOD {
            console.log("ExampleOracleSimple: PERIOD_NOT_ELAPSED");
        } else {
            // overflow is desired, casting never truncates
            // cumulative price is in (uq112x112 price * seconds) units so we simply wrap it after division by time elapsed
            self.price0Average = f64::from(price0Cumulative.checked_sub(self.price0CumulativeLast)?).checked_div(timeElapsed)?;
            self.price1Average = f64::from(price1Cumulative.checked_sub(self.price1CumulativeLast)?).checked_div(timeElapsed)?;

            self.price0CumulativeLast = price0Cumulative?;
            self.price1CumulativeLast = price1Cumulative?;
            self.blockTimestampLast = blockTimestamp?;
        }
    }

    pub fn consult(
        &self,
        token: &Pubkey,
        amountIn: f64
    ) -> f64 {
        if token == self.token0 {
            self.price0Average.checked_mul(amountIn)
        } else if token == self.token1 {
            self.price1Average.checked_mul(amountIn)
        } else {
            0.into()
        }
    }

}

#[cfg(test)]
mod tests {
    /* uses */
    const MODEL_ORACLE: Oracle = Oracle {

    };

    #[test]
    fn test_update() {

    }

    #[test]
    fn test_consult() {

    }
}