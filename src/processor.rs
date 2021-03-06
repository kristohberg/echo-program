use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use spl_token::state::Mint;

use crate::instruction::EchoInstruction;
use crate::state::{AuthorizedBufferHeader, VendingMachineBufferHeader, AUTH_BUFFER_HEADER_SIZE};
pub struct Processor {}

impl Processor {
    pub fn process_instruction(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = EchoInstruction::try_from_slice(instruction_data)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        match instruction {
            EchoInstruction::Echo { data } => {
                msg!("Echo account");
                let account_iter = &mut accounts.iter();
                let echo_buffer = next_account_info(account_iter)?;
                let buffer = &mut (*echo_buffer.data).borrow_mut();
                msg!("data_: {:?} ", data);
                if buffer.len() == 0 {
                    msg!("Account has data length of 0. Failing. ");
                    return Err(ProgramError::AccountDataTooSmall);
                }
                let bytes_to_copy = buffer.len();
                for index in 0..bytes_to_copy {
                    buffer[index] = data[index]
                }
                msg!(
                    "Successfully wrote {} bytes to account of size {}",
                    bytes_to_copy,
                    buffer.len()
                );
            }
            EchoInstruction::InitializeAuthorizedEcho {
                buffer_seed,
                buffer_size,
            } => {
                msg!("Initialize Authorized echo");
                if buffer_size <= AUTH_BUFFER_HEADER_SIZE {
                    msg!(
                        "Invalid buffer length {}, must be greater than header size {}",
                        buffer_size,
                        AUTH_BUFFER_HEADER_SIZE
                    );
                    return Err(ProgramError::InvalidArgument);
                }
                let account_iter = &mut accounts.iter();
                let authorized_buffer = next_account_info(account_iter)?;
                let authority = next_account_info(account_iter)?;
                let system_program = next_account_info(account_iter)?;

                let buffer_seed_b = buffer_seed.to_le_bytes();

                let (pubkey, bump_seed) = Pubkey::find_program_address(
                    &[b"authority", authority.key.as_ref(), &buffer_seed_b],
                    &_program_id,
                );

                if pubkey != *authorized_buffer.key {
                    msg!("authorized buffer is not a correct PDA");
                    return Err(ProgramError::InvalidAccountData);
                }

                // create pda
                let create_account_ix = system_instruction::create_account(
                    &authority.key,
                    &authorized_buffer.key,
                    Rent::get()?.minimum_balance(buffer_size),
                    buffer_size as u64,
                    _program_id,
                );

                invoke_signed(
                    &create_account_ix,
                    &[
                        authorized_buffer.clone(),
                        authority.clone(),
                        system_program.clone(),
                    ],
                    &[&[
                        b"authority",
                        authority.key.as_ref(),
                        &buffer_seed.to_le_bytes(),
                        &[bump_seed],
                    ]],
                )?;
                let buffer = &mut (*authorized_buffer.data).borrow_mut();
                let buffer_header = AuthorizedBufferHeader {
                    bump_seed,
                    buffer_seed,
                };

                buffer[0..AUTH_BUFFER_HEADER_SIZE]
                    .copy_from_slice(&buffer_header.try_to_vec().unwrap());
                msg!("Authorized buffer len: {}", buffer_size);
                msg!("Bump seed: {}", bump_seed);
                msg!("Buffer seed: {}", buffer_seed);
            }
            EchoInstruction::AuthorizedEcho { data } => {
                msg!("Authorized echo");
                let accounts_iter = &mut accounts.iter();
                let authorized_buffer = next_account_info(accounts_iter)?;
                let authority = next_account_info(accounts_iter)?;
                let buffer = &mut (*authorized_buffer.data).try_borrow_mut().unwrap();
                let buffer_header =
                    AuthorizedBufferHeader::try_from_slice(&buffer[..AUTH_BUFFER_HEADER_SIZE])
                        .unwrap();

                let pda = Pubkey::create_program_address(
                    &[
                        b"authority",
                        authority.key.as_ref(),
                        &buffer_header.buffer_seed.to_le_bytes(),
                        &[buffer_header.bump_seed],
                    ],
                    _program_id,
                )
                .unwrap();

                if pda != *authorized_buffer.key {
                    msg!("authorized buffer is not correct pda");
                    return Err(ProgramError::IllegalOwner);
                }

                let buffer_data = &mut buffer[AUTH_BUFFER_HEADER_SIZE..];

                for index in 0..buffer_data.len() {
                    buffer_data[index] = match index < data.len() {
                        true => data[index],
                        false => 0,
                    };
                }
            }
            EchoInstruction::InitializeVendingMachine { price, buffer_size } => {
                msg!("Initialize vending machine");
                let accounts_iter = &mut accounts.iter();
                let vending_machine_buffer = next_account_info(accounts_iter)?;
                let vending_machine_mint = next_account_info(accounts_iter)?;
                let payer = next_account_info(accounts_iter)?;
                let system_program = next_account_info(accounts_iter)?;

                msg!("price: {} , buffer_size: {}", price, buffer_size);

                let _mint =
                    Mint::unpack_unchecked(&vending_machine_mint.data.borrow()).map_err(|e| {
                        msg!("invalid account");
                        return e;
                    });

                let (pda, bump) = Pubkey::find_program_address(
                    &[
                        b"vending_machine",
                        vending_machine_mint.key.as_ref(),
                        &price.to_le_bytes(),
                    ],
                    _program_id,
                );

                if pda != *vending_machine_buffer.key {
                    msg!("vending machine buffer pubkey is not equal to expected pda");
                    return Err(ProgramError::InvalidAccountData);
                }

                let create_vending_machine_buffer = system_instruction::create_account(
                    payer.key,
                    &pda,
                    Rent::get()?.minimum_balance(buffer_size),
                    buffer_size as u64,
                    _program_id,
                );

                invoke_signed(
                    &create_vending_machine_buffer,
                    &[
                        payer.clone(),
                        system_program.clone(),
                        vending_machine_buffer.clone(),
                    ],
                    &[&[
                        b"vending_machine",
                        vending_machine_mint.key.as_ref(),
                        &price.to_le_bytes(),
                        &[bump],
                    ]],
                )?;

                let buffer = &mut (*vending_machine_buffer.data).borrow_mut();
                let vending_machine_buffer_header = VendingMachineBufferHeader {
                    bump_seed: bump,
                    price: price,
                };

                buffer[0..AUTH_BUFFER_HEADER_SIZE]
                    .copy_from_slice(&vending_machine_buffer_header.try_to_vec().unwrap());

                msg!("Vending machine buffer len: {}", buffer_size);
                msg!("Bump seed: {}", bump);
                msg!("Buffer price: {}", price);
            }
            _ => {
                msg!("invalid instruction");
                return Err(ProgramError::InvalidInstructionData);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use solana_program::clock::Epoch;
    use std::{borrow::Borrow, mem};

    // #[test]
    // fn test_initialize_authorize_echo() {
    //     let program_id = Pubkey::default();
    //     let authority = Pubkey::default();
    //     let mut lamports = 0;
    //     let mut authority_lamports = 10;
    //     let mut authority_data = vec![0; mem::size_of::<u32>()];
    //     let buffer_seed: u64 = 3;
    //     let buffer_size: usize = 8;
    //     let (authorized_key, bump) = Pubkey::find_program_address(
    //         &[b"authority", authority.as_ref(), &buffer_seed.to_le_bytes()],
    //         &program_id,
    //     );
    //     let mut data = vec![0; buffer_size];

    //     let authorization_echo_account = AccountInfo::new(
    //         &authorized_key,
    //         false,
    //         true,
    //         &mut lamports,
    //         &mut data,
    //         &authority,
    //         false,
    //         Epoch::default(),
    //     );

    //     let authority_account = AccountInfo::new(
    //         &authority,
    //         true,
    //         false,
    //         &mut authority_lamports,
    //         &mut authority_data,
    //         &program_id,
    //         false,
    //         Epoch::default(),
    //     );

    //     let system_program = AccountInfo::new(
    //         &system_program::id(),
    //         true,
    //         false,
    //         &mut 0,
    //         &mut [0; 1],
    //         &system_program::id(),
    //         false,
    //         Epoch::default(),
    //     );

    //     let accounts = vec![
    //         authorization_echo_account,
    //         authority_account,
    //         system_program,
    //     ];

    //     let mut instruction_data: Vec<u8> = Vec::new();
    //     let authorization_echo_instruction = EchoInstruction::InitializeAuthorizedEcho {
    //         buffer_seed,
    //         buffer_size,
    //     };
    //     authorization_echo_instruction
    //         .serialize(&mut instruction_data)
    //         .unwrap();

    //     Processor::process_instruction(&program_id, &accounts, &instruction_data).unwrap();
    //     println!("data: {:?}", (*accounts[0].data).borrow());
    // }
    // #[test]
    // fn test_sanity() {
    //     let program_id = Pubkey::default();
    //     let key = Pubkey::default();
    //     let mut lamports = 0;
    //     let mut data = vec![0; mem::size_of::<u32>()];
    //     let owner = Pubkey::default();

    //     let echo_account = AccountInfo::new(
    //         &key,
    //         false,
    //         true,
    //         &mut lamports,
    //         &mut data,
    //         &owner,
    //         false,
    //         Epoch::default(),
    //     );

    //     let mut instruction_data: Vec<u8> = Vec::new();
    //     //instruction_data.push(0);
    //     let echo_instruction = EchoInstruction::Echo {
    //         data: vec![0, 1, 1, 1, 2],
    //     };
    //     echo_instruction.serialize(&mut instruction_data).unwrap();

    //     let instruction = EchoInstruction::try_from_slice(&instruction_data);
    //     println!("{:?}", instruction);
    //     let accounts = vec![echo_account];

    //     Processor::process_instruction(&program_id, &accounts, &instruction_data).unwrap();

    //     println!("data. {:?}", (*accounts[0].data).borrow());
    //     assert!(false)
    // }
}
