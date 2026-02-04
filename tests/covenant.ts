import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Covenant } from "../target/types/covenant";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";

describe("Covenant Protocol", () => {
  // Configure the client to use the local cluster
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Covenant as Program<Covenant>;

  // Test accounts
  let protocolPda: PublicKey;
  let protocolBump: number;
  let providerPda: PublicKey;
  let providerBump: number;
  let vaultPda: PublicKey;
  let vaultBump: number;
  let slaPda: PublicKey;
  let slaBump: number;

  // Test keypairs
  const serviceProvider = Keypair.generate();
  const reporter = Keypair.generate();

  // Constants
  const MIN_STAKE = 0.1 * LAMPORTS_PER_SOL; // 0.1 SOL
  const STAKE_AMOUNT = 0.5 * LAMPORTS_PER_SOL; // 0.5 SOL

  before(async () => {
    // Airdrop SOL to test accounts
    const airdropProvider = await provider.connection.requestAirdrop(
      serviceProvider.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropProvider);

    const airdropReporter = await provider.connection.requestAirdrop(
      reporter.publicKey,
      1 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropReporter);

    // Derive PDAs
    [protocolPda, protocolBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("protocol")],
      program.programId
    );

    [providerPda, providerBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider"), serviceProvider.publicKey.toBuffer()],
      program.programId
    );

    [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), serviceProvider.publicKey.toBuffer()],
      program.programId
    );

    [slaPda, slaBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("sla"), providerPda.toBuffer()],
      program.programId
    );
  });

  describe("Protocol Initialization", () => {
    it("Initializes the protocol", async () => {
      const tx = await program.methods
        .initialize()
        .accounts({
          protocol: protocolPda,
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      console.log("Protocol initialized:", tx);

      // Verify protocol state
      const protocolAccount = await program.account.protocol.fetch(protocolPda);
      expect(protocolAccount.authority.toString()).to.equal(provider.wallet.publicKey.toString());
      expect(protocolAccount.totalProviders.toNumber()).to.equal(0);
      expect(protocolAccount.totalStaked.toNumber()).to.equal(0);
      expect(protocolAccount.totalSlashed.toNumber()).to.equal(0);
    });
  });

  describe("Provider Registration", () => {
    it("Registers a service provider with stake", async () => {
      const name = "TestAgent";
      const serviceEndpoint = "https://api.testagent.ai/v1";

      const tx = await program.methods
        .registerProvider(name, serviceEndpoint, new anchor.BN(STAKE_AMOUNT))
        .accounts({
          protocol: protocolPda,
          provider: providerPda,
          stakeVault: vaultPda,
          providerAuthority: serviceProvider.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([serviceProvider])
        .rpc();

      console.log("Provider registered:", tx);

      // Verify provider state
      const providerAccount = await program.account.provider.fetch(providerPda);
      expect(providerAccount.name).to.equal(name);
      expect(providerAccount.serviceEndpoint).to.equal(serviceEndpoint);
      expect(providerAccount.stakeAmount.toNumber()).to.equal(STAKE_AMOUNT);
      expect(providerAccount.violations.toNumber()).to.equal(0);
      expect(providerAccount.successfulRequests.toNumber()).to.equal(0);
      expect(providerAccount.isActive).to.equal(true);

      // Verify protocol stats updated
      const protocolAccount = await program.account.protocol.fetch(protocolPda);
      expect(protocolAccount.totalProviders.toNumber()).to.equal(1);
      expect(protocolAccount.totalStaked.toNumber()).to.equal(STAKE_AMOUNT);
    });

    it("Fails to register with insufficient stake", async () => {
      const insufficientProvider = Keypair.generate();

      // Airdrop just enough for transaction fees
      const airdrop = await provider.connection.requestAirdrop(
        insufficientProvider.publicKey,
        0.05 * LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdrop);

      const [insufficientProviderPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("provider"), insufficientProvider.publicKey.toBuffer()],
        program.programId
      );

      const [insufficientVaultPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), insufficientProvider.publicKey.toBuffer()],
        program.programId
      );

      try {
        await program.methods
          .registerProvider("LowStake", "https://lowstake.ai", new anchor.BN(MIN_STAKE / 2))
          .accounts({
            protocol: protocolPda,
            provider: insufficientProviderPda,
            stakeVault: insufficientVaultPda,
            providerAuthority: insufficientProvider.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([insufficientProvider])
          .rpc();

        expect.fail("Should have thrown InsufficientStake error");
      } catch (error) {
        expect(error.message).to.include("InsufficientStake");
      }
    });
  });

  describe("SLA Definition", () => {
    it("Defines SLA terms for a provider", async () => {
      const uptimeGuarantee = 95;      // 95%
      const maxResponseTimeMs = 2000;   // 2 seconds
      const accuracyGuarantee = 99;     // 99%
      const penaltyPercentage = 10;     // 10% slash per violation

      const tx = await program.methods
        .defineSla(uptimeGuarantee, maxResponseTimeMs, accuracyGuarantee, penaltyPercentage)
        .accounts({
          provider: providerPda,
          sla: slaPda,
          authority: serviceProvider.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([serviceProvider])
        .rpc();

      console.log("SLA defined:", tx);

      // Verify SLA state
      const slaAccount = await program.account.sla.fetch(slaPda);
      expect(slaAccount.provider.toString()).to.equal(providerPda.toString());
      expect(slaAccount.uptimeGuarantee).to.equal(uptimeGuarantee);
      expect(slaAccount.maxResponseTimeMs).to.equal(maxResponseTimeMs);
      expect(slaAccount.accuracyGuarantee).to.equal(accuracyGuarantee);
      expect(slaAccount.penaltyPercentage).to.equal(penaltyPercentage);
      expect(slaAccount.isActive).to.equal(true);
    });

    it("Fails to define SLA with invalid percentage", async () => {
      // Create a new provider for this test
      const newProvider = Keypair.generate();
      const airdrop = await provider.connection.requestAirdrop(
        newProvider.publicKey,
        2 * LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdrop);

      const [newProviderPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("provider"), newProvider.publicKey.toBuffer()],
        program.programId
      );

      const [newVaultPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), newProvider.publicKey.toBuffer()],
        program.programId
      );

      const [newSlaPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("sla"), newProviderPda.toBuffer()],
        program.programId
      );

      // First register the provider
      await program.methods
        .registerProvider("InvalidSLATest", "https://test.ai", new anchor.BN(STAKE_AMOUNT))
        .accounts({
          protocol: protocolPda,
          provider: newProviderPda,
          stakeVault: newVaultPda,
          providerAuthority: newProvider.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([newProvider])
        .rpc();

      // Try to define SLA with >100% uptime
      try {
        await program.methods
          .defineSla(101, 2000, 99, 10) // 101% uptime is invalid
          .accounts({
            provider: newProviderPda,
            sla: newSlaPda,
            authority: newProvider.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([newProvider])
          .rpc();

        expect.fail("Should have thrown InvalidPercentage error");
      } catch (error) {
        expect(error.message).to.include("InvalidPercentage");
      }
    });
  });

  describe("Recording Success", () => {
    it("Records a successful service request", async () => {
      const beforeProvider = await program.account.provider.fetch(providerPda);
      const beforeCount = beforeProvider.successfulRequests.toNumber();

      const tx = await program.methods
        .recordSuccess()
        .accounts({
          provider: providerPda,
          caller: provider.wallet.publicKey,
        })
        .rpc();

      console.log("Success recorded:", tx);

      const afterProvider = await program.account.provider.fetch(providerPda);
      expect(afterProvider.successfulRequests.toNumber()).to.equal(beforeCount + 1);
    });
  });

  describe("Violation Reporting & Slashing", () => {
    let violationPda: PublicKey;

    it("Reports an SLA violation", async () => {
      const providerAccount = await program.account.provider.fetch(providerPda);
      const violationIndex = providerAccount.violations.toNumber();

      [violationPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("violation"),
          providerPda.toBuffer(),
          new anchor.BN(violationIndex).toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      const evidenceHash = Buffer.alloc(32);
      evidenceHash.fill(1); // Mock evidence hash

      const tx = await program.methods
        .reportViolation(
          { uptimeViolation: {} },
          Array.from(evidenceHash),
          "Service was down for 30 minutes on 2024-02-04"
        )
        .accounts({
          provider: providerPda,
          violation: violationPda,
          reporter: reporter.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([reporter])
        .rpc();

      console.log("Violation reported:", tx);

      // Verify violation state
      const violationAccount = await program.account.violation.fetch(violationPda);
      expect(violationAccount.provider.toString()).to.equal(providerPda.toString());
      expect(violationAccount.reporter.toString()).to.equal(reporter.publicKey.toString());
      expect(violationAccount.isResolved).to.equal(false);

      // Verify provider violations incremented
      const updatedProvider = await program.account.provider.fetch(providerPda);
      expect(updatedProvider.violations.toNumber()).to.equal(violationIndex + 1);
    });

    it("Slashes provider stake for violation", async () => {
      const beforeProvider = await program.account.provider.fetch(providerPda);
      const beforeStake = beforeProvider.stakeAmount.toNumber();
      const beforeReporterBalance = await provider.connection.getBalance(reporter.publicKey);

      const slaAccount = await program.account.sla.fetch(slaPda);
      const expectedSlash = Math.floor((beforeStake * slaAccount.penaltyPercentage) / 100);

      const tx = await program.methods
        .slash()
        .accounts({
          protocol: protocolPda,
          provider: providerPda,
          sla: slaPda,
          violation: violationPda,
          stakeVault: vaultPda,
          reporter: reporter.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([reporter])
        .rpc();

      console.log("Provider slashed:", tx);

      // Verify provider stake reduced
      const afterProvider = await program.account.provider.fetch(providerPda);
      expect(afterProvider.stakeAmount.toNumber()).to.equal(beforeStake - expectedSlash);

      // Verify violation marked as resolved
      const violationAccount = await program.account.violation.fetch(violationPda);
      expect(violationAccount.isResolved).to.equal(true);

      // Verify protocol stats updated
      const protocolAccount = await program.account.protocol.fetch(protocolPda);
      expect(protocolAccount.totalSlashed.toNumber()).to.be.greaterThan(0);

      console.log(`Slashed ${expectedSlash / LAMPORTS_PER_SOL} SOL from provider`);
    });
  });

  describe("Stake Withdrawal", () => {
    it("Allows provider to withdraw partial stake", async () => {
      const beforeProvider = await program.account.provider.fetch(providerPda);
      const currentStake = beforeProvider.stakeAmount.toNumber();
      const withdrawAmount = currentStake - MIN_STAKE; // Withdraw down to minimum

      if (withdrawAmount > 0) {
        const tx = await program.methods
          .withdrawStake(new anchor.BN(withdrawAmount))
          .accounts({
            protocol: protocolPda,
            provider: providerPda,
            stakeVault: vaultPda,
            providerAuthority: serviceProvider.publicKey,
            authority: serviceProvider.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([serviceProvider])
          .rpc();

        console.log("Stake withdrawn:", tx);

        // Verify provider stake reduced
        const afterProvider = await program.account.provider.fetch(providerPda);
        expect(afterProvider.stakeAmount.toNumber()).to.equal(MIN_STAKE);
        expect(afterProvider.isActive).to.equal(true); // Still active with minimum stake
      }
    });
  });
});
