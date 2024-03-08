/// Entirely cargo culted from [SHA-crypt.txt](https://akkadia.org/drepper/SHA-crypt.txt), mirrored [here](../docs/SHA-crypt.txt).
use ring::digest;
use thiserror::Error;

const ROUNDS_MIN: u32 = 1000;
const ROUNDS_MAX: u32 = 999_999_999;
const ROUNDS_DEFAULT: u32 = 5000;

pub enum Sha2Mode {
	Sha256,
	Sha512,
}

impl Sha2Mode {
	/// Encodes the given data slice (A Sha digest), into a base64 string
	/// using the given mode. Returns none if the given data slice is not
	/// valid for the given mode (i.e. 32 bytes for sha256, 64 for sha512)
	fn crypt_sha2_base64(&self, data: &[u8]) -> String {
		// Each mode shuffles bytes differently
		// This table arranges the bytes into the proper order, as 4 tuples
		// where the first three values are the bytes, and the fourth is the number of characters
		// to extract from those bytes
		let bytes = match self {
			Sha2Mode::Sha256 => vec![
				(data[0], data[10], data[20], 4),
				(data[21], data[1], data[11], 4),
				(data[12], data[22], data[2], 4),
				(data[3], data[13], data[23], 4),
				(data[24], data[4], data[14], 4),
				(data[15], data[25], data[5], 4),
				(data[6], data[16], data[26], 4),
				(data[27], data[7], data[17], 4),
				(data[18], data[28], data[8], 4),
				(data[9], data[19], data[29], 4),
				(0, data[31], data[30], 3),
			],
			Sha2Mode::Sha512 => vec![
				(data[0], data[21], data[42], 4),
				(data[22], data[43], data[1], 4),
				(data[44], data[2], data[23], 4),
				(data[3], data[24], data[45], 4),
				(data[25], data[46], data[4], 4),
				(data[47], data[5], data[26], 4),
				(data[6], data[27], data[48], 4),
				(data[28], data[49], data[7], 4),
				(data[50], data[8], data[29], 4),
				(data[9], data[30], data[51], 4),
				(data[31], data[52], data[10], 4),
				(data[53], data[11], data[32], 4),
				(data[12], data[33], data[54], 4),
				(data[34], data[55], data[13], 4),
				(data[56], data[14], data[35], 4),
				(data[15], data[36], data[57], 4),
				(data[37], data[58], data[16], 4),
				(data[59], data[17], data[38], 4),
				(data[18], data[39], data[60], 4),
				(data[40], data[61], data[19], 4),
				(data[62], data[20], data[41], 4),
				(0, 0, data[63], 2),
			],
		};

		const B64_TABLE: &[u8; 64] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

		let mut encode = String::new();
		for (a, b, c, n) in bytes.into_iter() {
			let bytes = crypt_sha2_base64_bytes(a, b, c);
			encode.extend((0..n).map(|i| B64_TABLE[bytes[i] as usize] as char));
		}

		encode
	}

	/// Replicates the functionality of crypt(3), encoding a salt and password
	/// using the given Sha2 mode and returning the base64 hash.
	pub fn crypt_sha2(&self, salt: &[u8], password: &[u8], rounds: Option<u32>) -> Result<String, Sha2Error> {
		let rounds = rounds.unwrap_or(ROUNDS_DEFAULT);
		if !(ROUNDS_MIN..=ROUNDS_MAX).contains(&rounds) {
			return Err(Sha2Error::InvalidRounds(rounds));
		}

		let algorithm = match self {
			Sha2Mode::Sha256 => &digest::SHA256,
			Sha2Mode::Sha512 => &digest::SHA512,
		};

		// Based off of https://akkadia.org/drepper/SHA-crypt.txt

		//  start digest A.
		let mut digest_a = Vec::new();

		// the password string is added to digest A.
		digest_a.extend_from_slice(password);

		// the salt string is added to digest A.
		digest_a.extend_from_slice(salt);

		// start digest B.
		let mut digest_b = Vec::new();
		// add the password to digest B.
		digest_b.extend_from_slice(password);
		// add the salt string to digest B.
		digest_b.extend_from_slice(salt);
		// add the password again to digest B.
		digest_b.extend_from_slice(password);

		// finish digest B.
		let digest_b = digest::digest(algorithm, &digest_b);

		// For each block of 32 or 64 bytes in the password string (excluding
		// the terminating NUL in the C representation), add digest B to digest A.
		// For the remaining N bytes of the password string add the first
		// N bytes of digest B to digest A.
		digest_a.extend(digest_b.as_ref().iter().cycle().take(password.len()));

		// For each bit of the binary representation of the length of the
		// password string up to and including the highest 1-digit, starting
		// from to lowest bit position (numeric value 1):
		let mut len = password.len();
		while len > 0 {
			if len & 1 == 1 {
				// a) for a 1-digit add digest B to digest A.
				digest_a.extend(digest_b.as_ref());
			} else {
				// for a 0-digit add the password string.
				digest_a.extend_from_slice(password);
			}

			len >>= 1;
		}

		// finish digest A
		let digest_a = digest::digest(algorithm, &digest_a);

		// start digest DP
		let mut digest_dp = Vec::new();

		// for every byte in the password (excluding the terminating NUL byte
		// in the C representation of the string) add the password to digest DP.
		for _ in 0..password.len() {
			digest_dp.extend_from_slice(password);
		}

		// finish digest DP.
		let digest_dp = digest::digest(algorithm, &digest_dp);

		//  produce byte sequence P of the same length as the password where
		//  a) for each block of 32 or 64 bytes of length of the password string
		//  the entire digest DP is used
		//  b) for the remaining N (up to  31 or 63) bytes use the first N
		//     bytes of digest DP
		let p = digest_dp
			.as_ref()
			.iter()
			.cycle()
			.take(password.len())
			.collect::<Vec<_>>();

		// start digest DS
		let mut digest_ds = Vec::new();

		// repeat the following 16+A[0] times, where A[0] represents the first
		// byte in digest A interpreted as an 8-bit unsigned value add the salt to digest DS.
		for _ in 0..16 + digest_a.as_ref()[0] {
			digest_ds.extend_from_slice(salt);
		}

		// finish digest DS.
		let digest_ds = digest::digest(algorithm, &digest_ds);

		// produce byte sequence S of the same length as the salt string where
		// a) for each block of 32 or 64 bytes of length of the salt string the entire digest DS is used
		// b) for the remaining N (up to  31 or 63) bytes use the first N bytes of digest DS
		let s: Vec<u8> = digest_ds.as_ref().iter().cycle().take(salt.len()).cloned().collect();
		let mut previous_digest = digest_a;

		// repeat a loop according to the number specified in the rounds=<N>
		// specification in the salt (or the default value if none is
		// present).  Each round is numbered, starting with 0 and up to N-1.

		// The loop uses a digest as input.  In the first round it is the
		// digest A. In the latter steps it is the digest
		// produced in step 21.h of the previous round.  The following text
		// uses the notation "digest A/C" to describe this behavior.
		for round in 0..rounds {
			// start digest C
			let mut digest_c = Vec::new();

			if round % 2 == 1 {
				// for odd round numbers add the byte sequense P to digest C.
				digest_c.extend_from_slice(&p);
			} else {
				// for even round numbers add digest A/C.
				digest_c.extend(previous_digest.as_ref());
			}

			// for all round numbers not divisible by 3 add the byte sequence S.
			if round % 3 != 0 {
				digest_c.extend(&s);
			}

			// for all round numbers not divisible by 7 add the byte sequence P.
			if round % 7 != 0 {
				digest_c.extend_from_slice(&p);
			}

			if round % 2 == 1 {
				// for odd round numbers add digest A/C
				digest_c.extend(previous_digest.as_ref());
			} else {
				// for even round numbers add the byte sequence P
				digest_c.extend_from_slice(&p);
			}

			// finish digest C.
			let digest_c: Vec<u8> = digest_c.into_iter().cloned().collect();
			previous_digest = digest::digest(algorithm, &digest_c);
		}

		Ok(self.crypt_sha2_base64(previous_digest.as_ref()))
	}
}

/// Converts the given 3 bytes into 4 offsets in the Base64 Table.
fn crypt_sha2_base64_bytes(a: u8, b: u8, c: u8) -> [u8; 4] {
	let w = ((a as u32) << 16) | ((b as u32) << 8) | (c as u32);
	[
		(w & 0b111111) as u8,
		((w >> 6) & 0b111111) as u8,
		((w >> 12) & 0b111111) as u8,
		((w >> 18) & 0b111111) as u8,
	]
}

#[derive(Debug, Error)]
pub enum Sha2Error {
	#[error("Invalid rounds: {0}")]
	InvalidRounds(u32),
}

#[cfg(test)]
mod test {
	use super::Sha2Mode;

	#[test]
	fn test() {
		let digest = Sha2Mode::Sha512.crypt_sha2(b"GkbfJlFNcqp8VGNn", b"test", None).unwrap();

		assert_eq!(
			digest,
			"9uWgXkCpoCCdoER/1yc1on8Rus0.eQHfLWkGth30liq9rL.joqL1hP/KfBXUHNT8fbwB44Txr1A01WoozxokQ/"
		);
	}
}
