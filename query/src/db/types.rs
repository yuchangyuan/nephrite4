use postgres_types::*;
use std::error::Error;

use std::collections::{BTreeMap, BTreeSet};
use bytes::{BytesMut, BufMut};

#[derive(Debug)]
pub struct TsVector {
    pub data: BTreeMap<String, BTreeSet<(u8, u16)>>
}

impl ToSql for TsVector {
    fn to_sql(&self,
              _ty: &Type,
              buf: &mut BytesMut)
              -> Result<IsNull, Box<dyn Error + Sync + Send>> {

        let dat = &self.data;

        /*
        from http://www.npgsql.org/dev/types.html
        UInt32 number of lexemes
        for each lexeme:
          lexeme text in client encoding, null-terminated
          UInt16 number of positions
          for each position:
             UInt16 WordEntryPos,
             where the most significant 2 bits is weight,
             and the 14 least significant bits is pos (can't be 0).
             Weights 3,2,1,0 represent A,B,C,D
         */

        buf.put_u32(dat.len() as u32);

        for (lex, pset) in dat.iter() {
            buf.put_slice(lex.as_bytes());
            buf.put_u8(0);

            buf.put_u16(pset.len() as u16);

            // should put in order for number
            let mut pos: Vec<u16> = pset
                .iter()
                .map(|p| ((p.0 as u16 & 3) << 14) | (p.1 & 0x3fff))
                .collect();

            pos.sort();

            for p in pos.into_iter() {
                buf.put_u16(p);
            }
        };

        Ok(IsNull::No)
    }

    accepts!(TS_VECTOR);
    to_sql_checked!();
}
