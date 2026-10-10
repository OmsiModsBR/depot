# Correção: ZIP64 malicioso derrubava o jogo (omsi-cfg, `vfs.rs`)

Data: 2026-10-10. Achado B1 da auditoria de segurança. Sem commit ainda.

**Problema.** O montador de zip próprio lia `count` (nº de entradas) do registro ZIP64 do próprio arquivo e fazia
`Vec::with_capacity(count)`. Um valor como 2^64-1 causava `capacity overflow` (panic) ou alocação gigante, e o jogo caía
ao montar um `.zip` de mod. O tamanho declarado de cada entrada (até 2 GiB) e o tamanho comprimido também reservavam
memória antes de existir tal quantidade de bytes no arquivo.

**Correção.**
- A capacidade da lista de entradas é limitada a `tamanho_do_diretório / 46` (cada entrada ocupa pelo menos 46 bytes).
- A capacidade do buffer de descompressão é limitada a `compactado × 1032 + 64` (razão máxima do deflate).
- `read_at` (Unix e não-Unix) agora confere que o intervalo cabe no arquivo antes de alocar o buffer.
- Teste novo `a_zip64_record_with_a_huge_entry_count_is_survived`. Verifiquei que ele falha (`capacity overflow`) sem a correção e passa com ela.
  `cargo test -p omsi-cfg vfs`: 5 testes ok.

**Não coberto.** `MAX_ENTRY` continua 2 GiB: um zip-bomba legítimo de deflate ainda pode gastar RAM ao descompactar (item separado).

## Diff

```diff
diff --git a/crates/omsi-cfg/src/vfs.rs b/crates/omsi-cfg/src/vfs.rs
index eccf298..2bc60eb 100644
--- a/crates/omsi-cfg/src/vfs.rs
+++ b/crates/omsi-cfg/src/vfs.rs
@@ -131,7 +131,7 @@ impl ZipArchive {
             return Err(bad(format!("{}: central directory out of range", path.display())));
         }
         let cd = archive.read_at(cd_offset, cd_size as usize)?;
-        let mut raw: Vec<(String, ZipEntry, bool)> = Vec::with_capacity(count as usize);
+        let mut raw: Vec<(String, ZipEntry, bool)> = Vec::with_capacity((count as usize).min(cd.len() / 46));
         let mut p = 0usize;
         while p + 46 <= cd.len() && le32(&cd, p) == 0x0201_4b50 {
             let flags = le16(&cd, p + 8);
@@ -258,6 +258,7 @@ impl ZipArchive {
     #[cfg(unix)]
     fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
         use std::os::unix::fs::FileExt;
+        self.check_range(offset, len)?;
         let mut buf = vec![0u8; len];
         self.file.read_exact_at(&mut buf, offset)?;
         Ok(buf)
@@ -266,6 +267,7 @@ impl ZipArchive {
     #[cfg(not(unix))]
     fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
         use std::io::Seek;
+        self.check_range(offset, len)?;
         let _g = self.lock.lock().unwrap();
         let mut f = &self.file;
         f.seek(io::SeekFrom::Start(offset))?;
@@ -274,6 +276,16 @@ impl ZipArchive {
         Ok(buf)
     }
 
+    /// A read of `len` bytes at `offset` must lie inside the file: sizes in an archive are the
+    /// archive's own claims, and a buffer is only made for bytes that can exist.
+    fn check_range(&self, offset: u64, len: usize) -> io::Result<()> {
+        let file_len = self.file.metadata()?.len();
+        if offset.checked_add(len as u64).is_none_or(|end| end > file_len) {
+            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, format!("{}: read past the end of the archive", self.path.display())));
+        }
+        Ok(())
+    }
+
     /// The archive file (= the mount point).
     pub fn path(&self) -> &Path {
         &self.path
@@ -312,7 +324,7 @@ impl ZipArchive {
         let out = match e.method {
             0 => packed,
             8 => {
-                let mut out = Vec::with_capacity(e.size as usize);
+                let mut out = Vec::with_capacity((e.size as usize).min((e.compressed as usize).saturating_mul(1032).saturating_add(64)));
                 flate2::read::DeflateDecoder::new(&packed[..]).take(e.size + 1).read_to_end(&mut out)?;
                 out
             }
@@ -508,6 +520,36 @@ mod tests {
     use super::*;
     use std::io::Write;
 
+    /// A ZIP64 end record that claims 2^64-1 entries must not panic or reserve memory for them.
+    #[test]
+    fn a_zip64_record_with_a_huge_entry_count_is_survived() {
+        let mut z: Vec<u8> = Vec::new();
+        // ZIP64 end of central directory record (56 bytes): count, size and offset of the directory
+        z.extend_from_slice(&0x0606_4b50u32.to_le_bytes());
+        z.extend_from_slice(&44u64.to_le_bytes());
+        z.extend_from_slice(&[45, 0, 45, 0]);
+        z.extend_from_slice(&0u32.to_le_bytes());
+        z.extend_from_slice(&0u32.to_le_bytes());
+        z.extend_from_slice(&u64::MAX.to_le_bytes());
+        z.extend_from_slice(&u64::MAX.to_le_bytes());
+        z.extend_from_slice(&0u64.to_le_bytes());
+        z.extend_from_slice(&0u64.to_le_bytes());
+        // locator pointing at it, then the classic end record
+        z.extend_from_slice(&0x0706_4b50u32.to_le_bytes());
+        z.extend_from_slice(&0u32.to_le_bytes());
+        z.extend_from_slice(&0u64.to_le_bytes());
+        z.extend_from_slice(&1u32.to_le_bytes());
+        z.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
+        z.extend_from_slice(&[0; 18]);
+        let dir = std::env::temp_dir().join(format!("depot-zip64-{}", std::process::id()));
+        std::fs::create_dir_all(&dir).unwrap();
+        let path = dir.join("huge-count.zip");
+        std::fs::write(&path, &z).unwrap();
+        // (an error or an empty archive are both fine; a panic or a huge reservation is not)
+        let _ = ZipArchive::open(&path);
+        let _ = std::fs::remove_dir_all(&dir);
+    }
+
     /// A small archive with a wrapper folder, a stored and a deflated entry.
     fn build_zip(path: &Path) {
         let files: Vec<(&str, Vec<u8>, bool)> = vec![
```
