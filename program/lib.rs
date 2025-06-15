use anchor_lang::prelude::*;
use anchor_lang::error_code;
use anchor_lang::solana_program::hash::hash;

declare_id!("GwY9aAMD8nxhZxuTtPBbsFfgiqsVGkRTeA5fRyDjNkdM");

#[program]
pub mod session_clicker {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let game: &mut Account<Game> = &mut ctx.accounts.game;
        let player: &Signer = &ctx.accounts.player;
        
        game.player = *player.key;
        game.total_clicks = 0;
        game.last_session_end = Clock::get()?.unix_timestamp;
        
        Ok(())
    }

    pub fn start_session(ctx: Context<StartSession>, commitment: [u8; 32]) -> Result<()> {
        let game: &mut Account<Game> = &mut ctx.accounts.game;
        let session: &mut Account<Session> = &mut ctx.accounts.session;
        
        // Verify player ownership
        if &game.player != ctx.accounts.player.key {
            return Err(error!(ClickerError::InvalidPlayer));
        }

        // Check if there's already an active session
        if game.active_session.is_some() {
            return Err(error!(ClickerError::SessionAlreadyActive));
        }

        let current_time = Clock::get()?.unix_timestamp;
        
        session.player = *ctx.accounts.player.key;
        session.game = game.key();
        session.commitment = commitment;
        session.start_time = current_time;
        session.revealed = false;
        
        game.active_session = Some(session.key());
        
        Ok(())
    }

    pub fn end_session(
        ctx: Context<EndSession>, 
        clicks: u32, 
        nonce: u64,
        max_session_duration: i64
    ) -> Result<()> {
        let game: &mut Account<Game> = &mut ctx.accounts.game;
        let session: &mut Account<Session> = &mut ctx.accounts.session;
        
        // Verify player ownership
        if &game.player != ctx.accounts.player.key {
            return Err(error!(ClickerError::InvalidPlayer));
        }

        // Verify this is the active session
        if game.active_session != Some(session.key()) {
            return Err(error!(ClickerError::InvalidSession));
        }

        // Check if session is already revealed
        if session.revealed {
            return Err(error!(ClickerError::SessionAlreadyRevealed));
        }

        let current_time = Clock::get()?.unix_timestamp;
        let session_duration = current_time - session.start_time;

        // Enforce maximum session duration (prevents infinite offline clicking)
        if session_duration > max_session_duration {
            return Err(error!(ClickerError::SessionTooLong));
        }

        // Verify the commitment matches the revealed values
        let mut data_to_hash = Vec::new();
        data_to_hash.extend_from_slice(&clicks.to_le_bytes());
        data_to_hash.extend_from_slice(&nonce.to_le_bytes());
        data_to_hash.extend_from_slice(ctx.accounts.player.key.as_ref());
        
        let revealed_hash = hash(&data_to_hash).to_bytes();

        if revealed_hash != session.commitment {
            return Err(error!(ClickerError::InvalidCommitment));
        }

        // Enforce reasonable clicking rate (e.g., max 10 clicks per second)
        let max_clicks = (session_duration as u32) * 10; // 10 CPS max
        if clicks > max_clicks {
            return Err(error!(ClickerError::UnrealisticClickRate));
        }

        // Update game state
        game.total_clicks += clicks as u64;
        game.last_session_end = current_time;
        game.active_session = None;

        // Mark session as revealed
        session.revealed = true;
        session.actual_clicks = clicks;
        session.end_time = current_time;

        Ok(())
    }

    pub fn cancel_session(ctx: Context<CancelSession>) -> Result<()> {
        let game: &mut Account<Game> = &mut ctx.accounts.game;
        let session: &mut Account<Session> = &mut ctx.accounts.session;
        
        // Verify player ownership
        if &game.player != ctx.accounts.player.key {
            return Err(error!(ClickerError::InvalidPlayer));
        }

        // Verify this is the active session
        if game.active_session != Some(session.key()) {
            return Err(error!(ClickerError::InvalidSession));
        }

        // Clear active session
        game.active_session = None;
        
        // Mark session as cancelled (no clicks awarded)
        session.revealed = true;
        session.actual_clicks = 0;
        session.end_time = Clock::get()?.unix_timestamp;

        Ok(())
    }
}

#[account]
#[derive(Default)]
pub struct Game {
    player: Pubkey,                    // 32 bytes
    total_clicks: u64,                 // 8 bytes  
    last_session_end: i64,             // 8 bytes
    active_session: Option<Pubkey>,    // 1 + 32 bytes
}

impl Game {
    pub const MAXIMUM_SIZE: usize = 32 + 8 + 8 + 1 + 32;
}

#[account]
#[derive(Default)]
pub struct Session {
    player: Pubkey,         // 32 bytes
    game: Pubkey,           // 32 bytes
    commitment: [u8; 32],   // 32 bytes - hash of (clicks, nonce, player)
    start_time: i64,        // 8 bytes
    end_time: i64,          // 8 bytes
    actual_clicks: u32,     // 4 bytes
    revealed: bool,         // 1 byte
}

impl Session {
    pub const MAXIMUM_SIZE: usize = 32 + 32 + 32 + 8 + 8 + 4 + 1;
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = player, space = 8 + Game::MAXIMUM_SIZE)]
    pub game: Account<'info, Game>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StartSession<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    #[account(init, payer = player, space = 8 + Session::MAXIMUM_SIZE)]
    pub session: Account<'info, Session>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct EndSession<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    #[account(mut)]
    pub session: Account<'info, Session>,
    pub player: Signer<'info>,
}

#[derive(Accounts)]
pub struct CancelSession<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    #[account(mut)]
    pub session: Account<'info, Session>,
    pub player: Signer<'info>,
}

#[error_code]
pub enum ClickerError {
    InvalidPlayer,
    SessionAlreadyActive,
    InvalidSession,
    SessionAlreadyRevealed,
    SessionTooLong,
    InvalidCommitment,
    UnrealisticClickRate,
}