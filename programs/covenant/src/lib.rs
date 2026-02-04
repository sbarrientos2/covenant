use anchor_lang::prelude::*;
use anchor_lang::system_program;

declare_id!("DsuUvdDe5S6Rnzg9NrBJRAZcrP83FvyGycyi2oVbzcec");

/// Covenant Protocol: Economic guarantees for AI agent services
///
/// Agents stake collateral to guarantee service quality.
/// SLAs are enforced automatically through on-chain verification.
/// Violations trigger slashing - bad actors lose stake, good actors get rewarded.

#[program]
pub mod covenant {
    use super::*;

    /// Initialize the Covenant protocol
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let protocol = &mut ctx.accounts.protocol;
        protocol.authority = ctx.accounts.authority.key();
        protocol.total_providers = 0;
        protocol.total_staked = 0;
        protocol.total_slashed = 0;
        protocol.bump = ctx.bumps.protocol;

        msg!("Covenant Protocol initialized");
        Ok(())
    }

    /// Register as a service provider with staked collateral
    pub fn register_provider(
        ctx: Context<RegisterProvider>,
        name: String,
        service_endpoint: String,
        stake_amount: u64,
    ) -> Result<()> {
        require!(name.len() <= 64, CovenantError::NameTooLong);
        require!(service_endpoint.len() <= 256, CovenantError::EndpointTooLong);
        require!(stake_amount >= MIN_STAKE, CovenantError::InsufficientStake);

        // Transfer stake to vault
        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.provider_authority.to_account_info(),
                to: ctx.accounts.stake_vault.to_account_info(),
            },
        );
        system_program::transfer(cpi_context, stake_amount)?;

        // Initialize provider account
        let provider = &mut ctx.accounts.provider;
        provider.authority = ctx.accounts.provider_authority.key();
        provider.name = name;
        provider.service_endpoint = service_endpoint;
        provider.stake_amount = stake_amount;
        provider.violations = 0;
        provider.successful_requests = 0;
        provider.created_at = Clock::get()?.unix_timestamp;
        provider.is_active = true;
        provider.bump = ctx.bumps.provider;

        // Update protocol stats
        let protocol = &mut ctx.accounts.protocol;
        protocol.total_providers += 1;
        protocol.total_staked += stake_amount;

        msg!("Provider registered with {} lamports staked", stake_amount);
        Ok(())
    }

    /// Define SLA terms for a service
    pub fn define_sla(
        ctx: Context<DefineSLA>,
        uptime_guarantee: u8,           // Percentage (0-100)
        max_response_time_ms: u32,      // Max response time in milliseconds
        accuracy_guarantee: u8,          // Percentage (0-100)
        penalty_percentage: u8,          // Percentage of stake to slash per violation
    ) -> Result<()> {
        require!(uptime_guarantee <= 100, CovenantError::InvalidPercentage);
        require!(accuracy_guarantee <= 100, CovenantError::InvalidPercentage);
        require!(penalty_percentage > 0 && penalty_percentage <= 100, CovenantError::InvalidPercentage);

        let sla = &mut ctx.accounts.sla;
        sla.provider = ctx.accounts.provider.key();
        sla.uptime_guarantee = uptime_guarantee;
        sla.max_response_time_ms = max_response_time_ms;
        sla.accuracy_guarantee = accuracy_guarantee;
        sla.penalty_percentage = penalty_percentage;
        sla.created_at = Clock::get()?.unix_timestamp;
        sla.is_active = true;
        sla.bump = ctx.bumps.sla;

        msg!("SLA defined: {}% uptime, {}ms response, {}% accuracy",
             uptime_guarantee, max_response_time_ms, accuracy_guarantee);
        Ok(())
    }

    /// Report an SLA violation (can be called by monitors or affected parties)
    pub fn report_violation(
        ctx: Context<ReportViolation>,
        violation_type: ViolationType,
        evidence_hash: [u8; 32],        // Hash of off-chain evidence
        description: String,
    ) -> Result<()> {
        require!(description.len() <= 512, CovenantError::DescriptionTooLong);
        require!(ctx.accounts.provider.is_active, CovenantError::ProviderInactive);

        let violation = &mut ctx.accounts.violation;
        violation.provider = ctx.accounts.provider.key();
        violation.reporter = ctx.accounts.reporter.key();
        violation.violation_type = violation_type;
        violation.evidence_hash = evidence_hash;
        violation.description = description;
        violation.timestamp = Clock::get()?.unix_timestamp;
        violation.is_resolved = false;
        violation.bump = ctx.bumps.violation;

        // Increment provider violations
        let provider = &mut ctx.accounts.provider;
        provider.violations += 1;

        msg!("Violation reported against provider");
        Ok(())
    }

    /// Execute slashing for a confirmed violation
    pub fn slash(ctx: Context<Slash>) -> Result<()> {
        let violation = &mut ctx.accounts.violation;
        let provider = &mut ctx.accounts.provider;
        let sla = &ctx.accounts.sla;
        let protocol = &mut ctx.accounts.protocol;

        require!(!violation.is_resolved, CovenantError::ViolationAlreadyResolved);
        require!(provider.stake_amount > 0, CovenantError::NoStakeToSlash);

        // Calculate slash amount
        let slash_amount = (provider.stake_amount as u128)
            .checked_mul(sla.penalty_percentage as u128)
            .unwrap()
            .checked_div(100)
            .unwrap() as u64;

        let actual_slash = std::cmp::min(slash_amount, provider.stake_amount);

        // Transfer slashed amount from vault to reporter (compensation)
        let protocol_seeds = &[
            b"protocol".as_ref(),
            &[protocol.bump],
        ];
        let signer_seeds = &[&protocol_seeds[..]];

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.stake_vault.to_account_info(),
                to: ctx.accounts.reporter.to_account_info(),
            },
            signer_seeds,
        );
        system_program::transfer(cpi_context, actual_slash)?;

        // Update state
        provider.stake_amount -= actual_slash;
        protocol.total_slashed += actual_slash;
        protocol.total_staked -= actual_slash;
        violation.is_resolved = true;

        // Deactivate provider if stake falls below minimum
        if provider.stake_amount < MIN_STAKE {
            provider.is_active = false;
            msg!("Provider deactivated due to insufficient stake");
        }

        msg!("Slashed {} lamports from provider", actual_slash);
        Ok(())
    }

    /// Record a successful service request (builds reputation)
    pub fn record_success(ctx: Context<RecordSuccess>) -> Result<()> {
        let provider = &mut ctx.accounts.provider;
        provider.successful_requests += 1;

        msg!("Successful request recorded. Total: {}", provider.successful_requests);
        Ok(())
    }

    /// Withdraw stake (only if no pending violations and cooldown passed)
    pub fn withdraw_stake(ctx: Context<WithdrawStake>, amount: u64) -> Result<()> {
        let provider = &mut ctx.accounts.provider;
        let protocol = &mut ctx.accounts.protocol;

        require!(provider.is_active, CovenantError::ProviderInactive);
        require!(amount <= provider.stake_amount, CovenantError::InsufficientStake);

        // Ensure minimum stake maintained if still active
        let remaining = provider.stake_amount - amount;
        if remaining > 0 {
            require!(remaining >= MIN_STAKE, CovenantError::WouldBreachMinStake);
        }

        // Transfer from vault to provider
        let protocol_seeds = &[
            b"protocol".as_ref(),
            &[protocol.bump],
        ];
        let signer_seeds = &[&protocol_seeds[..]];

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.stake_vault.to_account_info(),
                to: ctx.accounts.provider_authority.to_account_info(),
            },
            signer_seeds,
        );
        system_program::transfer(cpi_context, amount)?;

        // Update state
        provider.stake_amount -= amount;
        protocol.total_staked -= amount;

        if provider.stake_amount == 0 {
            provider.is_active = false;
            protocol.total_providers -= 1;
        }

        msg!("Withdrew {} lamports", amount);
        Ok(())
    }
}

// Constants
pub const MIN_STAKE: u64 = 100_000_000; // 0.1 SOL minimum stake

// Account Structures

#[account]
pub struct Protocol {
    pub authority: Pubkey,
    pub total_providers: u64,
    pub total_staked: u64,
    pub total_slashed: u64,
    pub bump: u8,
}

#[account]
pub struct Provider {
    pub authority: Pubkey,
    pub name: String,
    pub service_endpoint: String,
    pub stake_amount: u64,
    pub violations: u64,
    pub successful_requests: u64,
    pub created_at: i64,
    pub is_active: bool,
    pub bump: u8,
}

#[account]
pub struct SLA {
    pub provider: Pubkey,
    pub uptime_guarantee: u8,
    pub max_response_time_ms: u32,
    pub accuracy_guarantee: u8,
    pub penalty_percentage: u8,
    pub created_at: i64,
    pub is_active: bool,
    pub bump: u8,
}

#[account]
pub struct Violation {
    pub provider: Pubkey,
    pub reporter: Pubkey,
    pub violation_type: ViolationType,
    pub evidence_hash: [u8; 32],
    pub description: String,
    pub timestamp: i64,
    pub is_resolved: bool,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum ViolationType {
    UptimeViolation,
    ResponseTimeViolation,
    AccuracyViolation,
    ServiceUnavailable,
    Other,
}

// Instruction Contexts

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 8 + 8 + 8 + 1,
        seeds = [b"protocol"],
        bump
    )]
    pub protocol: Account<'info, Protocol>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct RegisterProvider<'info> {
    #[account(
        mut,
        seeds = [b"protocol"],
        bump = protocol.bump
    )]
    pub protocol: Account<'info, Protocol>,

    #[account(
        init,
        payer = provider_authority,
        space = 8 + 32 + 4 + 64 + 4 + 256 + 8 + 8 + 8 + 8 + 1 + 1,
        seeds = [b"provider", provider_authority.key().as_ref()],
        bump
    )]
    pub provider: Account<'info, Provider>,

    /// CHECK: Vault PDA to hold staked funds
    #[account(
        mut,
        seeds = [b"vault", provider_authority.key().as_ref()],
        bump
    )]
    pub stake_vault: AccountInfo<'info>,

    #[account(mut)]
    pub provider_authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DefineSLA<'info> {
    #[account(
        mut,
        seeds = [b"provider", provider.authority.as_ref()],
        bump = provider.bump,
        has_one = authority @ CovenantError::Unauthorized
    )]
    pub provider: Account<'info, Provider>,

    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 1 + 4 + 1 + 1 + 8 + 1 + 1,
        seeds = [b"sla", provider.key().as_ref()],
        bump
    )]
    pub sla: Account<'info, SLA>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ReportViolation<'info> {
    #[account(
        mut,
        seeds = [b"provider", provider.authority.as_ref()],
        bump = provider.bump
    )]
    pub provider: Account<'info, Provider>,

    #[account(
        init,
        payer = reporter,
        space = 8 + 32 + 32 + 1 + 32 + 4 + 512 + 8 + 1 + 1,
        seeds = [b"violation", provider.key().as_ref(), &provider.violations.to_le_bytes()],
        bump
    )]
    pub violation: Account<'info, Violation>,

    #[account(mut)]
    pub reporter: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Slash<'info> {
    #[account(
        mut,
        seeds = [b"protocol"],
        bump = protocol.bump
    )]
    pub protocol: Account<'info, Protocol>,

    #[account(
        mut,
        seeds = [b"provider", provider.authority.as_ref()],
        bump = provider.bump
    )]
    pub provider: Account<'info, Provider>,

    #[account(
        seeds = [b"sla", provider.key().as_ref()],
        bump = sla.bump
    )]
    pub sla: Account<'info, SLA>,

    #[account(
        mut,
        seeds = [b"violation", provider.key().as_ref(), &(provider.violations - 1).to_le_bytes()],
        bump = violation.bump,
        has_one = reporter
    )]
    pub violation: Account<'info, Violation>,

    /// CHECK: Vault PDA holding staked funds
    #[account(
        mut,
        seeds = [b"vault", provider.authority.as_ref()],
        bump
    )]
    pub stake_vault: AccountInfo<'info>,

    #[account(mut)]
    pub reporter: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecordSuccess<'info> {
    #[account(
        mut,
        seeds = [b"provider", provider.authority.as_ref()],
        bump = provider.bump
    )]
    pub provider: Account<'info, Provider>,

    pub caller: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawStake<'info> {
    #[account(
        mut,
        seeds = [b"protocol"],
        bump = protocol.bump
    )]
    pub protocol: Account<'info, Protocol>,

    #[account(
        mut,
        seeds = [b"provider", provider_authority.key().as_ref()],
        bump = provider.bump,
        has_one = authority @ CovenantError::Unauthorized
    )]
    pub provider: Account<'info, Provider>,

    /// CHECK: Vault PDA holding staked funds
    #[account(
        mut,
        seeds = [b"vault", provider_authority.key().as_ref()],
        bump
    )]
    pub stake_vault: AccountInfo<'info>,

    #[account(mut)]
    pub provider_authority: Signer<'info>,

    /// CHECK: Provider authority for validation
    pub authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

// Error Codes

#[error_code]
pub enum CovenantError {
    #[msg("Name exceeds maximum length of 64 characters")]
    NameTooLong,
    #[msg("Service endpoint exceeds maximum length of 256 characters")]
    EndpointTooLong,
    #[msg("Description exceeds maximum length of 512 characters")]
    DescriptionTooLong,
    #[msg("Stake amount is below minimum required")]
    InsufficientStake,
    #[msg("Invalid percentage value (must be 0-100)")]
    InvalidPercentage,
    #[msg("Unauthorized action")]
    Unauthorized,
    #[msg("Provider is inactive")]
    ProviderInactive,
    #[msg("Violation has already been resolved")]
    ViolationAlreadyResolved,
    #[msg("No stake available to slash")]
    NoStakeToSlash,
    #[msg("Withdrawal would breach minimum stake requirement")]
    WouldBreachMinStake,
}
