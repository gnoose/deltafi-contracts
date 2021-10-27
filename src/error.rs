//! Error types

use num_derive::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    msg,
    program_error::{PrintProgramError, ProgramError},
};
use thiserror::Error;

/// Errors that may be returned by the TokenSwap program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum SwapError {
    // 0
    /// The account cannot be initialized because it is already being used.
    #[error("Swap account already in use")]
    AlreadyInUse,
    /// The address of the admin fee account is incorrect.
    #[error("Address of the admin fee account is incorrect")]
    InvalidAdmin,
    /// Active admin transfer in progress
    #[error("Active admin transfer in progress")]
    ActiveTransfer,
    /// No active admin transfer in progress
    #[error("No active admin transfer in progress")]
    NoActiveTransfer,
    /// Admin transfer deadline exceeded
    #[error("Admin transfer deadline exceeded")]
    AdminDeadlineExceeded,

    // 5
    /// Unauthorized
    #[error("Account is not authorized to execute this instruction")]
    Unauthorized,
    /// The account is not owned by program
    #[error("Input account owner is not the program")]
    InvalidAccountOwner,
    /// The owner of the input isn't set to the program address generated by the program.
    #[error("Input account owner is not the program address")]
    InvalidOwner,
    /// The input account must be a signer.
    #[error("Input account must be signer")]
    InvalidSigner,
    /// The owner of the pool token output is set to the program address generated by the program.
    #[error("Output pool account owner cannot be the program address")]
    InvalidOutputOwner,

    // 10
    /// Address of the provided swap token account is incorrect.
    #[error("Address of the provided swap token account is incorrect")]
    IncorrectSwapAccount,
    /// The program address provided doesn't match the value generated by the program.
    #[error("Invalid program address generated from nonce and key")]
    InvalidProgramAddress,
    /// The provided token account has a close authority.
    #[error("Token account has a close authority")]
    InvalidCloseAuthority,
    /// The pool token mint has a freeze authority.
    #[error("Pool token mint has a freeze authority")]
    InvalidFreezeAuthority,
    /// Incorrect token program ID
    #[error("Incorrect token program ID")]
    IncorrectTokenProgramId,

    // 15
    /// Address of the provided token mint is incorrect
    #[error("Address of the provided token mint is incorrect")]
    IncorrectMint,
    /// The deserialization of the account returned something besides State::Mint.
    #[error("Deserialized account is not an SPL Token mint")]
    ExpectedMint,
    /// Swap input token accounts have the same mint
    #[error("Swap input token accounts have the same mint")]
    RepeatedMint,
    /// The deserialization of the account returned something besides State::Account.
    #[error("Deserialized account is not an SPL Token account")]
    ExpectedAccount,
    /// Invalid instruction number passed in.
    #[error("Invalid instruction")]
    InvalidInstruction,

    // 20
    /// Instruction unpack failed.
    #[error("Instruction unpack is failed")]
    InstructionUnpackError,
    /// The pool supply is empty.
    #[error("Pool token supply is 0")]
    EmptyPool,
    /// The input token account is empty.
    #[error("Input token account empty")]
    EmptySupply,
    /// The pool token mint has a non-zero supply.
    #[error("Pool token mint has a non-zero supply")]
    InvalidSupply,
    /// The provided token account has a delegate.
    #[error("Token account has a delegate")]
    InvalidDelegate,

    // 25
    /// The input token is invalid for swap.
    #[error("InvalidInput")]
    InvalidInput,
    /// Swap pool is paused
    #[error("Swap pool is paused")]
    IsPaused,
    /// Lamport balance below rent-exempt threshold.
    #[error("Lamport balance below rent-exempt threshold")]
    NotRentExempt,
    /// The calculation failed.
    #[error("CalculationFailure")]
    CalculationFailure,
    /// Swap instruction exceeds desired slippage limit
    #[error("Swap instruction exceeds desired slippage limit")]
    ExceededSlippage,

    // 30
    /// Token mint decimals must be the same.
    #[error("Token mints must have same decimals")]
    MismatchedDecimals,
    /// Oracle config is invalid
    #[error("Input oracle config is invalid")]
    InvalidOracleConfig,
    /// Insufficient liquidity amount to withdraw
    #[error("Insufficient liquidity available")]
    InsufficientLiquidity,
    /// User has no liquidity position
    #[error("User has no liquidity position")]
    LiquidityPositionEmpty,
    /// Invalid position key
    #[error("Invalid position key")]
    InvalidPositionKey,

    // 35
    /// Invalid claim timestamp
    #[error("Invalid claim timestamp")]
    InvalidClaimTime,
    /// Insufficient claim amount
    #[error("Insufficient claim amount")]
    InsufficientClaimAmount,
    /// Insufficient funds
    #[error("Insufficient funds")]
    InsufficientFunds,
    /// Withdraw tokens not enough
    #[error("Withdraw not enough")]
    WithdrawNotEnough,
}
impl From<SwapError> for ProgramError {
    fn from(e: SwapError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for SwapError {
    fn type_of() -> &'static str {
        "Swap Error"
    }
}

impl PrintProgramError for SwapError {
    fn print<E>(&self)
    where
        E: 'static
            + std::error::Error
            + DecodeError<E>
            + PrintProgramError
            + num_traits::FromPrimitive,
    {
        match self {
            SwapError::AlreadyInUse => msg!("Error: Swap account already in use"),
            SwapError::InvalidAdmin => {
                msg!("Error: Address of the admin fee account is incorrect")
            }
            SwapError::ActiveTransfer => msg!("Error: Active admin transfer in progress"),
            SwapError::NoActiveTransfer => msg!("Error: No active admin transfer in progress"),
            SwapError::AdminDeadlineExceeded => msg!("Error: Admin transfer deadline exceeded"),
            SwapError::Unauthorized => {
                msg!("Error: Account is not authorized to execute this instruction")
            }
            SwapError::InvalidAccountOwner => {
                msg!("Error: Input account owner is not the program address")
            }
            SwapError::InvalidOwner => {
                msg!("Error: The input account owner is not the program address")
            }
            SwapError::InvalidSigner => msg!("Error: Ivanlid signer account provided"),
            SwapError::InvalidOutputOwner => {
                msg!("Error: Output pool account owner cannot be the program address")
            }
            SwapError::IncorrectSwapAccount => {
                msg!("Error: Address of the provided swap token account is incorrect")
            }
            SwapError::InvalidProgramAddress => {
                msg!("Error: Invalid program address generated from nonce and key")
            }
            SwapError::InvalidCloseAuthority => msg!("Error: Token account has a close authority"),
            SwapError::InvalidFreezeAuthority => {
                msg!("Error: Pool token mint has a freeze authority")
            }
            SwapError::IncorrectTokenProgramId => {
                msg!("Error: The provided token program does not match the token program expected by the swap")
            }
            SwapError::IncorrectMint => {
                msg!("Error: Address of the provided token mint is incorrect")
            }
            SwapError::ExpectedMint => {
                msg!("Error: Deserialized account is not an SPL Token mint")
            }
            SwapError::RepeatedMint => msg!("Error: Swap input token accounts have the same mint"),
            SwapError::ExpectedAccount => {
                msg!("Error: Deserialized account is not an SPL Token account")
            }
            SwapError::InvalidInstruction => msg!("Error: InvalidInstruction"),
            SwapError::InstructionUnpackError => msg!("Error: Instruction unpacking is failed"),
            SwapError::EmptySupply => msg!("Error: Input token account empty"),
            SwapError::EmptyPool => msg!("Error: Pool token supply is 0"),
            SwapError::InvalidSupply => msg!("Error: Pool token mint has a non-zero supply"),
            SwapError::InvalidDelegate => msg!("Error: Token account has a delegate"),
            SwapError::InvalidInput => msg!("Error: InvalidInput"),
            SwapError::IsPaused => msg!("Error: Swap pool is paused"),
            SwapError::NotRentExempt => msg!("Error: Lamport balance below rent-exempt threshold"),
            SwapError::CalculationFailure => msg!("Error: CalculationFailure"),
            SwapError::ExceededSlippage => {
                msg!("Error: Swap instruction exceeds desired slippage limit")
            }
            SwapError::MismatchedDecimals => msg!("Error: Token mints must have same decimals"),
            SwapError::InvalidOracleConfig => msg!("Error: Failed to sync oracle"),
            SwapError::InsufficientLiquidity => msg!("Error: Insufficient liquidity available"),
            SwapError::LiquidityPositionEmpty => msg!("Error: Do not have liquidity in the pool"),
            SwapError::InvalidPositionKey => msg!("Error: Invalid liquidity position key"),
            SwapError::InvalidClaimTime => {
                msg!("Error: User cannot cliam rewards before agreed time")
            }
            SwapError::InsufficientClaimAmount => msg!("Error: No rewards to claim"),
            SwapError::InsufficientFunds => msg!("Error: Insufficient funds to trade"),
            SwapError::WithdrawNotEnough => msg!("Error: Exceeded number of liquidity to withdraw"),
        }
    }
}
