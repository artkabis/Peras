;; The buffers of Peras, shared by every allocator: arrays and strings.
;;
;; Included by build.rs into an allocator module, which provides `$alloc`,
;; `$release`, `$raise`, and the globals `$TOO_LARGE` and `$DEEP`.

  ;; Buffers: the arrays and strings of Peras. A buffer is a 16-byte block, its
  ;; header, which keeps the same address when the buffer grows:
  ;;
  ;;   +0 length   +4 capacity, both counted in elements   +8 elements, or 0
  ;;   +12 the size of an element in bytes, plus DEEP when its elements are
  ;;       buffers themselves, which it owns: an array of strings
  ;;
  ;; The elements are a block of their own, `capacity * size` bytes, except
  ;; in a small buffer whose elements are not buffers, a short string: they
  ;; follow its header in the same block, one allocation instead of two, and
  ;; +12 carries INLINE while they are there. When such a buffer grows, its
  ;; elements move to a block of their own, INLINE goes, and the room after
  ;; the header stays unused until the header is released. Every `size` argument below may carry DEEP, and INLINE, which
  ;; `buf_new` ignores; only its low byte is a size.

  (global $INLINE i32 (i32.const 0x200))
  ;; An array of views (`str_split_views`): its elements are headers in its
  ;; own block, whose bytes belong to the string it was split from. Releasing
  ;; it releases that block only; a copy of it is an ordinary DEEP array.
  (global $VIEWS i32 (i32.const 0x400))
  ;; The most bytes of elements kept in the header's block.
  (global $INLINE_MAX i32 (i32.const 256))

  ;; A new, empty buffer with room for `cap` elements of `size` bytes.
  (func $buf_new (export "buf_new") (param $cap i32) (param $size i32) (param $site i32) (result i32)
    (local $h i32) (local $bytes i32)
    (local.set $size (i32.and (local.get $size) (i32.xor (global.get $INLINE) (i32.const -1))))
    ;; A new array made from an array of views owns its strings.
    (if (i32.and (local.get $size) (global.get $VIEWS))
      (then (local.set $size (i32.or
        (i32.and (local.get $size) (i32.xor (global.get $VIEWS) (i32.const -1)))
        (global.get $DEEP)))))
    (local.set $bytes (call $bytes (local.get $cap) (local.get $size) (local.get $site)))
    (if (i32.and
          (i32.and (i32.ne (local.get $cap) (i32.const 0))
            (i32.le_u (local.get $bytes) (global.get $INLINE_MAX)))
          (i32.eqz (i32.and (local.get $size) (global.get $DEEP))))
      (then
        (local.set $h (call $alloc (i32.add (i32.const 16) (local.get $bytes)) (local.get $site)))
        (i32.store offset=8 (local.get $h) (i32.add (local.get $h) (i32.const 16)))
        (local.set $size (i32.or (local.get $size) (global.get $INLINE))))
      (else
        (local.set $h (call $alloc (i32.const 16) (local.get $site)))
        (i32.store offset=8 (local.get $h)
          (if (result i32) (i32.eqz (local.get $cap))
            (then (i32.const 0))
            (else (call $alloc (local.get $bytes) (local.get $site)))))))
    (i32.store (local.get $h) (i32.const 0))
    (i32.store offset=4 (local.get $h) (local.get $cap))
    (i32.store offset=12 (local.get $h) (local.get $size))
    (local.get $h))

  ;; Whether the elements of `h` follow its header, in its block.
  (func $inline (param $h i32) (result i32)
    (i32.ne (i32.and (i32.load offset=12 (local.get $h)) (global.get $INLINE)) (i32.const 0)))

  ;; `count * size`, or a fault when it exceeds what one block can hold.
  (func $bytes (param $count i32) (param $size i32) (param $site i32) (result i32)
    (local $wide i64)
    (local.set $wide (i64.mul (i64.extend_i32_u (local.get $count))
      (i64.extend_i32_u (i32.and (local.get $size) (i32.const 0xFF)))))
    (if (i64.gt_u (local.get $wide) (i64.const 0x40000000))
      (then (call $raise (global.get $TOO_LARGE) (i32.const 0x40000001) (local.get $site) (i32.const 0))))
    (i32.wrap_i64 (local.get $wide)))

  ;; Makes room for `extra` more elements: the capacity at least doubles, so
  ;; adding elements one by one costs a constant time each, on average.
  (func $buf_reserve (export "buf_reserve") (param $h i32) (param $extra i32) (param $size i32) (param $site i32)
    (local $len i32) (local $cap i32) (local $need i64) (local $new_cap i32) (local $data i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $cap (i32.load offset=4 (local.get $h)))
    (local.set $need (i64.add (i64.extend_i32_u (local.get $len)) (i64.extend_i32_u (local.get $extra))))
    (if (i64.le_u (local.get $need) (i64.extend_i32_u (local.get $cap)))
      (then (return)))
    (if (i64.gt_u (local.get $need) (i64.const 0x40000000))
      (then (call $raise (global.get $TOO_LARGE) (i32.const 0x40000001) (local.get $site) (i32.const 0))))
    (local.set $new_cap (i32.shl (local.get $cap) (i32.const 1)))
    (if (i32.lt_u (local.get $new_cap) (i32.wrap_i64 (local.get $need)))
      (then (local.set $new_cap (i32.wrap_i64 (local.get $need)))))
    (if (i32.lt_u (local.get $new_cap) (i32.const 4))
      (then (local.set $new_cap (i32.const 4))))
    (local.set $data
      (call $alloc (call $bytes (local.get $new_cap) (local.get $size) (local.get $site)) (local.get $site)))
    (if (i32.load offset=8 (local.get $h))
      (then
        (memory.copy (local.get $data) (i32.load offset=8 (local.get $h))
          (call $bytes (local.get $len) (local.get $size) (local.get $site)))
        ;; Elements in the header's block stay there, unused, until the
        ;; header is released; the buffer is no longer INLINE.
        (if (call $inline (local.get $h))
          (then (i32.store offset=12 (local.get $h)
            (i32.and (i32.load offset=12 (local.get $h)) (i32.xor (global.get $INLINE) (i32.const -1)))))
          (else (call $release (i32.load offset=8 (local.get $h)) (local.get $site))))))
    (i32.store offset=4 (local.get $h) (local.get $new_cap))
    (i32.store offset=8 (local.get $h) (local.get $data)))

  ;; Releases a buffer: its header first, which catches a buffer released twice
  ;; before its poisoned contents are read, then the buffers it owns, if its
  ;; elements are buffers, then its elements.
  (func $buf_release (export "buf_release") (param $h i32) (param $site i32)
    (local $data i32) (local $len i32) (local $deep i32) (local $i i32) (local $shape i32)
    ;; A map: its header first, which catches a block released twice, then
    ;; its keys and blocks, from what the header held.
    (if (i32.and (i32.load offset=12 (local.get $h)) (global.get $MAP))
      (then
        (local.set $data (i32.load offset=8 (local.get $h)))
        (local.set $len (i32.load offset=4 (local.get $h)))
        (local.set $deep (i32.and (i32.load offset=12 (local.get $h)) (global.get $STRKEY)))
        (local.set $i (i32.load offset=20 (local.get $h)))
        (local.set $shape (i32.load offset=28 (local.get $h)))
        (call $release (local.get $h) (local.get $site))
        (call $map_free (local.get $data) (local.get $len) (local.get $deep) (local.get $i)
          (local.get $shape))
        (return)))
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $len (i32.load (local.get $h)))
    (local.set $deep (i32.and (i32.load offset=12 (local.get $h)) (global.get $DEEP)))
    ;; Elements in the header's block are released with it.
    (if (call $inline (local.get $h))
      (then (local.set $data (i32.const 0))))
    (call $release (local.get $h) (local.get $site))
    (if (local.get $deep)
      (then
        (block $done
          (loop $next
            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
            (call $buf_release
              (i32.load (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 2))))
              (local.get $site))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next)))))
    (if (local.get $data)
      (then (call $release (local.get $data) (local.get $site)))))

  ;; A copy of a buffer, with copies of the buffers it owns.
  (func $buf_clone (export "buf_clone") (param $h i32) (param $site i32) (result i32)
    (local $copy i32) (local $len i32) (local $size i32) (local $i i32) (local $at i32)
    ;; A map: its own copy, keys and values included.
    (if (i32.and (i32.load offset=12 (local.get $h)) (global.get $MAP))
      (then (return (call $map_clone (local.get $h) (local.get $site)))))
    (local.set $len (i32.load (local.get $h)))
    (local.set $size (i32.load offset=12 (local.get $h)))
    (local.set $copy (call $buf_new (local.get $len) (local.get $size) (local.get $site)))
    (if (local.get $len)
      (then
        (memory.copy (i32.load offset=8 (local.get $copy)) (i32.load offset=8 (local.get $h))
          (call $bytes (local.get $len) (local.get $size) (local.get $site)))))
    (i32.store (local.get $copy) (local.get $len))
    (if (i32.and (local.get $size) (i32.or (global.get $DEEP) (global.get $VIEWS)))
      (then
        (block $done
          (loop $next
            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
            (local.set $at (i32.add (i32.load offset=8 (local.get $copy)) (i32.shl (local.get $i) (i32.const 2))))
            (i32.store (local.get $at) (call $buf_clone (i32.load (local.get $at)) (local.get $site)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next)))))
    (local.get $copy))

  ;; Strings: buffers of bytes, in UTF-8. Every index and length counts bytes;
  ;; an index that falls inside a character moves back to its start. Each
  ;; function lends its operands and gives a new buffer, owned by the caller.

  ;; A new string of the `len` bytes at `src`.
  (func $str_from (param $src i32) (param $len i32) (result i32)
    (local $h i32)
    (local.set $h (call $buf_new (local.get $len) (i32.const 1) (i32.const 0)))
    (if (local.get $len)
      (then (memory.copy (i32.load offset=8 (local.get $h)) (local.get $src) (local.get $len))))
    (i32.store (local.get $h) (local.get $len))
    (local.get $h))

  ;; Adds the `len` bytes at `src` at the end of the string `h`.
  (func $str_add (param $h i32) (param $src i32) (param $len i32)
    (if (i32.eqz (local.get $len)) (then (return)))
    (call $buf_reserve (local.get $h) (local.get $len) (i32.const 1) (i32.const 0))
    (memory.copy
      (i32.add (i32.load offset=8 (local.get $h)) (i32.load (local.get $h)))
      (local.get $src) (local.get $len))
    (i32.store (local.get $h) (i32.add (i32.load (local.get $h)) (local.get $len))))

  ;; Whether the `n` bytes at `a` and at `b` are the same.
  (func $same (param $a i32) (param $b i32) (param $n i32) (result i32)
    (local $i i32)
    (block $differ
      (loop $next
        (if (i32.ge_u (local.get $i) (local.get $n)) (then (return (i32.const 1))))
        (br_if $differ (i32.ne
          (i32.load8_u (i32.add (local.get $a) (local.get $i)))
          (i32.load8_u (i32.add (local.get $b) (local.get $i)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.const 0))

  ;; Whether byte `i` of the string `h` continues a character.
  (func $continues (param $h i32) (param $i i32) (result i32)
    (i32.eq
      (i32.and (i32.load8_u (i32.add (i32.load offset=8 (local.get $h)) (local.get $i))) (i32.const 0xC0))
      (i32.const 0x80)))

  ;; An index of `h` made valid: negative counts from the end, then clamped
  ;; to the length, then moved back to the start of its character.
  (func $index (param $h i32) (param $i i32) (result i32)
    (local $len i32)
    (local.set $len (i32.load (local.get $h)))
    (if (i32.lt_s (local.get $i) (i32.const 0))
      (then (local.set $i (i32.add (local.get $i) (local.get $len)))))
    (if (i32.lt_s (local.get $i) (i32.const 0)) (then (local.set $i (i32.const 0))))
    (if (i32.gt_s (local.get $i) (local.get $len)) (then (local.set $i (local.get $len))))
    (block $done
      (loop $back
        (br_if $done (i32.or (i32.eqz (local.get $i)) (i32.ge_u (local.get $i) (local.get $len))))
        (br_if $done (i32.eqz (call $continues (local.get $h) (local.get $i))))
        (local.set $i (i32.sub (local.get $i) (i32.const 1)))
        (br $back)))
    (local.get $i))

  ;; The first index at or after `from` where `needle` is in `h`, or -1.
  (func $str_index_of (export "str_index_of") (param $h i32) (param $needle i32) (param $from i32) (result i32)
    (local $len i32) (local $n i32) (local $i i32) (local $data i32) (local $first i32) (local $last i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $n (i32.load (local.get $needle)))
    (local.set $i (local.get $from))
    (if (i32.lt_s (local.get $i) (i32.const 0)) (then (local.set $i (i32.const 0))))
    ;; An empty needle is found where the search starts, if within the text.
    (if (i32.eqz (local.get $n))
      (then (return (select (local.get $i) (i32.const -1)
        (i32.le_u (local.get $i) (local.get $len))))))
    (if (i32.gt_u (local.get $n) (local.get $len)) (then (return (i32.const -1))))
    ;; The last start where the needle fits; its first byte, looked for byte
    ;; by byte, and the rest compared only where it matches.
    (local.set $last (i32.sub (local.get $len) (local.get $n)))
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $first (i32.load8_u (i32.load offset=8 (local.get $needle))))
    (block $none
      (loop $next
        (br_if $none (i32.gt_u (local.get $i) (local.get $last)))
        (if (i32.eq (i32.load8_u (i32.add (local.get $data) (local.get $i))) (local.get $first))
          (then
            (if (call $same
                  (i32.add (i32.add (local.get $data) (local.get $i)) (i32.const 1))
                  (i32.add (i32.load offset=8 (local.get $needle)) (i32.const 1))
                  (i32.sub (local.get $n) (i32.const 1)))
              (then (return (local.get $i))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.const -1))

  (func (export "str_starts_with") (param $h i32) (param $prefix i32) (result i32)
    (if (result i32) (i32.gt_u (i32.load (local.get $prefix)) (i32.load (local.get $h)))
      (then (i32.const 0))
      (else (call $same (i32.load offset=8 (local.get $h)) (i32.load offset=8 (local.get $prefix))
        (i32.load (local.get $prefix))))))

  (func (export "str_ends_with") (param $h i32) (param $suffix i32) (result i32)
    (local $len i32) (local $n i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $n (i32.load (local.get $suffix)))
    (if (result i32) (i32.gt_u (local.get $n) (local.get $len))
      (then (i32.const 0))
      (else (call $same
        (i32.add (i32.load offset=8 (local.get $h)) (i32.sub (local.get $len) (local.get $n)))
        (i32.load offset=8 (local.get $suffix)) (local.get $n)))))

  ;; The bytes from `start` to `end`.
  (func (export "str_slice") (param $h i32) (param $start i32) (param $end i32) (result i32)
    (local.set $start (call $index (local.get $h) (local.get $start)))
    (local.set $end (call $index (local.get $h) (local.get $end)))
    (if (i32.lt_s (local.get $end) (local.get $start)) (then (local.set $end (local.get $start))))
    (call $str_from (i32.add (i32.load offset=8 (local.get $h)) (local.get $start))
      (i32.sub (local.get $end) (local.get $start))))

  ;; Whether a byte is a space, a tab or a line break.
  (func $blank (param $b i32) (result i32)
    (i32.or (i32.or (i32.eq (local.get $b) (i32.const 32)) (i32.eq (local.get $b) (i32.const 9)))
      (i32.or (i32.eq (local.get $b) (i32.const 10)) (i32.eq (local.get $b) (i32.const 13)))))

  (func (export "str_trim") (param $h i32) (result i32)
    (local $data i32) (local $a i32) (local $b i32)
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $b (i32.load (local.get $h)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $a) (local.get $b)))
        (br_if $done (i32.eqz (call $blank (i32.load8_u (i32.add (local.get $data) (local.get $a))))))
        (local.set $a (i32.add (local.get $a) (i32.const 1)))
        (br $next)))
    (block $done
      (loop $next
        (br_if $done (i32.le_u (local.get $b) (local.get $a)))
        (br_if $done (i32.eqz (call $blank
          (i32.load8_u (i32.add (local.get $data) (i32.sub (local.get $b) (i32.const 1)))))))
        (local.set $b (i32.sub (local.get $b) (i32.const 1)))
        (br $next)))
    (call $str_from (i32.add (local.get $data) (local.get $a)) (i32.sub (local.get $b) (local.get $a))))

  ;; A copy with the ASCII letters from `lo` to `lo + 25` shifted by `by`.
  (func $str_case (param $h i32) (param $lo i32) (param $by i32) (result i32)
    (local $copy i32) (local $at i32) (local $end i32) (local $b i32)
    (local.set $copy (call $str_from (i32.load offset=8 (local.get $h)) (i32.load (local.get $h))))
    (local.set $at (i32.load offset=8 (local.get $copy)))
    (local.set $end (i32.add (local.get $at) (i32.load (local.get $copy))))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $at) (local.get $end)))
        (local.set $b (i32.load8_u (local.get $at)))
        (if (i32.lt_u (i32.sub (local.get $b) (local.get $lo)) (i32.const 26))
          (then (i32.store8 (local.get $at) (i32.add (local.get $b) (local.get $by)))))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (br $next)))
    (local.get $copy))

  (func (export "str_upper") (param $h i32) (result i32)
    (call $str_case (local.get $h) (i32.const 97) (i32.const -32)))

  (func (export "str_lower") (param $h i32) (result i32)
    (call $str_case (local.get $h) (i32.const 65) (i32.const 32)))

  (func (export "str_repeat") (param $h i32) (param $n i32) (result i32)
    (local $out i32) (local $i i32) (local $len i32)
    (local.set $len (i32.load (local.get $h)))
    (if (i32.lt_s (local.get $n) (i32.const 0)) (then (local.set $n (i32.const 0))))
    ;; `n * len` bytes; more than one block can hold is refused.
    (if (i64.gt_u (i64.mul (i64.extend_i32_u (local.get $n)) (i64.extend_i32_u (local.get $len)))
          (i64.const 0x40000000))
      (then (call $raise (global.get $TOO_LARGE) (i32.const 0x40000001) (i32.const 0) (i32.const 0))))
    (local.set $out (call $buf_new
      (i32.mul (local.get $n) (local.get $len)) (i32.const 1) (i32.const 0)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
        (call $str_add (local.get $out) (i32.load offset=8 (local.get $h)) (local.get $len))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.get $out))

  ;; `h` with `from` replaced by `to`: the first one, or all when `all`. An
  ;; empty `from` replaces nothing.
  (func $str_replace_in (param $h i32) (param $from i32) (param $to i32) (param $all i32) (result i32)
    (local $out i32) (local $i i32) (local $at i32) (local $n i32) (local $data i32)
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $n (i32.load (local.get $from)))
    (local.set $out (call $buf_new (i32.load (local.get $h)) (i32.const 1) (i32.const 0)))
    (block $done
      (loop $next
        (br_if $done (i32.eqz (local.get $n)))
        (local.set $at (call $str_index_of (local.get $h) (local.get $from) (local.get $i)))
        (br_if $done (i32.lt_s (local.get $at) (i32.const 0)))
        (call $str_add (local.get $out) (i32.add (local.get $data) (local.get $i))
          (i32.sub (local.get $at) (local.get $i)))
        (call $str_add (local.get $out) (i32.load offset=8 (local.get $to)) (i32.load (local.get $to)))
        (local.set $i (i32.add (local.get $at) (local.get $n)))
        (br_if $next (local.get $all))))
    (call $str_add (local.get $out) (i32.add (local.get $data) (local.get $i))
      (i32.sub (i32.load (local.get $h)) (local.get $i)))
    (local.get $out))

  (func (export "str_replace") (param $h i32) (param $from i32) (param $to i32) (result i32)
    (call $str_replace_in (local.get $h) (local.get $from) (local.get $to) (i32.const 0)))

  (func (export "str_replace_all") (param $h i32) (param $from i32) (param $to i32) (result i32)
    (call $str_replace_in (local.get $h) (local.get $from) (local.get $to) (i32.const 1)))

  ;; Adds the string `s` at the end of the array of strings `parts`.
  (func $push_part (param $parts i32) (param $s i32)
    (local $len i32)
    (call $buf_reserve (local.get $parts) (i32.const 1) (i32.or (i32.const 4) (global.get $DEEP)) (i32.const 0))
    (local.set $len (i32.load (local.get $parts)))
    (i32.store (i32.add (i32.load offset=8 (local.get $parts)) (i32.shl (local.get $len) (i32.const 2)))
      (local.get $s))
    (i32.store (local.get $parts) (i32.add (local.get $len) (i32.const 1))))

  ;; The parts of `h` between each `separator`; with an empty separator, its
  ;; characters.
  (func (export "str_split") (param $h i32) (param $separator i32) (result i32)
    (local $parts i32) (local $i i32) (local $j i32) (local $at i32) (local $n i32) (local $len i32) (local $data i32)
    (local.set $parts (call $buf_new (i32.const 0) (i32.or (i32.const 4) (global.get $DEEP)) (i32.const 0)))
    (local.set $len (i32.load (local.get $h)))
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $n (i32.load (local.get $separator)))
    (if (i32.eqz (local.get $n))
      (then
        (block $done
          (loop $next
            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
            (local.set $j (i32.add (local.get $i) (i32.const 1)))
            (block $end
              (loop $more
                (br_if $end (i32.ge_u (local.get $j) (local.get $len)))
                (br_if $end (i32.eqz (call $continues (local.get $h) (local.get $j))))
                (local.set $j (i32.add (local.get $j) (i32.const 1)))
                (br $more)))
            (call $push_part (local.get $parts)
              (call $str_from (i32.add (local.get $data) (local.get $i)) (i32.sub (local.get $j) (local.get $i))))
            (local.set $i (local.get $j))
            (br $next)))
        (return (local.get $parts))))
    (block $done
      (loop $next
        (local.set $at (call $str_index_of (local.get $h) (local.get $separator) (local.get $i)))
        (br_if $done (i32.lt_s (local.get $at) (i32.const 0)))
        (call $push_part (local.get $parts)
          (call $str_from (i32.add (local.get $data) (local.get $i)) (i32.sub (local.get $at) (local.get $i))))
        (local.set $i (i32.add (local.get $at) (local.get $n)))
        (br $next)))
    (call $push_part (local.get $parts)
      (call $str_from (i32.add (local.get $data) (local.get $i)) (i32.sub (local.get $len) (local.get $i))))
    (local.get $parts))

  ;; The parts of `h` between each `separator`, as `str_split` gives them, but
  ;; as views: one block holds a handle per part, then a 16-byte header per
  ;; part whose bytes are `h`'s own, flagged INLINE so that nothing releases
  ;; them. The array is VIEWS. The compiler gives it only where it proves `h`
  ;; outlives it and does not change meanwhile; a copy of a part, or of the
  ;; array, is an ordinary string. With an empty separator, its characters.
  (func (export "str_split_views") (param $h i32) (param $separator i32) (result i32)
    (local $parts i32) (local $block i32) (local $headers i32) (local $n i32) (local $c i32)
    (local $i i32) (local $j i32) (local $at i32) (local $k i32) (local $len i32) (local $data i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $k (i32.load (local.get $separator)))
    ;; A separator of one byte, the usual case: plain loops over the bytes.
    (if (i32.eq (local.get $k) (i32.const 1))
      (then (return (call $split_byte (local.get $data) (local.get $len)
        (i32.load8_u (i32.load offset=8 (local.get $separator)))))))
    ;; How many parts: the characters, or one more than the separators.
    (if (i32.eqz (local.get $k))
      (then
        (block $counted
          (loop $next
            (br_if $counted (i32.ge_u (local.get $i) (local.get $len)))
            (if (i32.eqz (call $continues (local.get $h) (local.get $i)))
              (then (local.set $n (i32.add (local.get $n) (i32.const 1)))))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next))))
      (else
        (local.set $n (i32.const 1))
        (block $counted
          (loop $next
            (local.set $at (call $str_index_of (local.get $h) (local.get $separator) (local.get $i)))
            (br_if $counted (i32.lt_s (local.get $at) (i32.const 0)))
            (local.set $n (i32.add (local.get $n) (i32.const 1)))
            (local.set $i (i32.add (local.get $at) (local.get $k)))
            (br $next)))))
    (local.set $parts (call $alloc (i32.const 16) (i32.const 0)))
    (if (local.get $n)
      (then (local.set $block
        (call $alloc (call $bytes (local.get $n) (i32.const 20) (i32.const 0)) (i32.const 0)))))
    (i32.store (local.get $parts) (local.get $n))
    (i32.store offset=4 (local.get $parts) (local.get $n))
    (i32.store offset=8 (local.get $parts) (local.get $block))
    (i32.store offset=12 (local.get $parts) (i32.or (i32.const 4) (global.get $VIEWS)))
    (local.set $headers (i32.add (local.get $block) (i32.shl (local.get $n) (i32.const 2))))
    ;; The same walk again, each part a header.
    (local.set $i (i32.const 0))
    (if (i32.eqz (local.get $k))
      (then
        (block $done
          (loop $next
            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
            (local.set $j (i32.add (local.get $i) (i32.const 1)))
            (block $end
              (loop $more
                (br_if $end (i32.ge_u (local.get $j) (local.get $len)))
                (br_if $end (i32.eqz (call $continues (local.get $h) (local.get $j))))
                (local.set $j (i32.add (local.get $j) (i32.const 1)))
                (br $more)))
            (call $view (local.get $block) (local.get $headers) (local.get $c)
              (i32.add (local.get $data) (local.get $i)) (i32.sub (local.get $j) (local.get $i)))
            (local.set $c (i32.add (local.get $c) (i32.const 1)))
            (local.set $i (local.get $j))
            (br $next)))
        (return (local.get $parts))))
    (block $done
      (loop $next
        (local.set $at (call $str_index_of (local.get $h) (local.get $separator) (local.get $i)))
        (br_if $done (i32.lt_s (local.get $at) (i32.const 0)))
        (call $view (local.get $block) (local.get $headers) (local.get $c)
          (i32.add (local.get $data) (local.get $i)) (i32.sub (local.get $at) (local.get $i)))
        (local.set $c (i32.add (local.get $c) (i32.const 1)))
        (local.set $i (i32.add (local.get $at) (local.get $k)))
        (br $next)))
    (call $view (local.get $block) (local.get $headers) (local.get $c)
      (i32.add (local.get $data) (local.get $i)) (i32.sub (local.get $len) (local.get $i)))
    (local.get $parts))

  ;; @simd
  ;; `str_split_views` on the `len` bytes at `data`, by the byte `sep`, with
  ;; WebAssembly SIMD: sixteen bytes compared at once, the separators found
  ;; in the bit mask of each sixteen, then the bytes left one at a time.
  (func $split_byte (param $data i32) (param $len i32) (param $sep i32) (result i32)
    (local $parts i32) (local $block i32) (local $hdr i32) (local $handle i32)
    (local $n i32) (local $i i32) (local $start i32) (local $end i32)
    (local $splat v128) (local $m i32) (local $at i32)
    (local.set $end (i32.add (local.get $data) (local.get $len)))
    (local.set $splat (i8x16.splat (local.get $sep)))
    (local.set $n (i32.const 1))
    (local.set $i (local.get $data))
    (block $wide
      (loop $next
        (br_if $wide (i32.gt_u (i32.add (local.get $i) (i32.const 16)) (local.get $end)))
        (local.set $n (i32.add (local.get $n)
          (i32.popcnt (i8x16.bitmask (i8x16.eq (v128.load align=1 (local.get $i)) (local.get $splat))))))
        (local.set $i (i32.add (local.get $i) (i32.const 16)))
        (br $next)))
    (block $counted
      (loop $next
        (br_if $counted (i32.ge_u (local.get $i) (local.get $end)))
        (if (i32.eq (i32.load8_u (local.get $i)) (local.get $sep))
          (then (local.set $n (i32.add (local.get $n) (i32.const 1)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.set $parts (call $alloc (i32.const 16) (i32.const 0)))
    (local.set $block
      (call $alloc (call $bytes (local.get $n) (i32.const 20) (i32.const 0)) (i32.const 0)))
    (i32.store (local.get $parts) (local.get $n))
    (i32.store offset=4 (local.get $parts) (local.get $n))
    (i32.store offset=8 (local.get $parts) (local.get $block))
    (i32.store offset=12 (local.get $parts) (i32.or (i32.const 4) (global.get $VIEWS)))
    (local.set $handle (local.get $block))
    (local.set $hdr (i32.add (local.get $block) (i32.shl (local.get $n) (i32.const 2))))
    (local.set $start (local.get $data))
    (local.set $i (local.get $data))
    (block $wide
      (loop $next
        (br_if $wide (i32.gt_u (i32.add (local.get $i) (i32.const 16)) (local.get $end)))
        (local.set $m (i8x16.bitmask (i8x16.eq (v128.load align=1 (local.get $i)) (local.get $splat))))
        ;; Each separator of the sixteen, lowest first, ends a part.
        (block $chunk
          (loop $bit
            (br_if $chunk (i32.eqz (local.get $m)))
            (local.set $at (i32.add (local.get $i) (i32.ctz (local.get $m))))
            (i32.store (local.get $hdr) (i32.sub (local.get $at) (local.get $start)))
            (i32.store offset=4 (local.get $hdr) (i32.sub (local.get $at) (local.get $start)))
            (i32.store offset=8 (local.get $hdr) (local.get $start))
            (i32.store offset=12 (local.get $hdr) (i32.or (i32.const 1) (global.get $INLINE)))
            (i32.store (local.get $handle) (local.get $hdr))
            (local.set $handle (i32.add (local.get $handle) (i32.const 4)))
            (local.set $hdr (i32.add (local.get $hdr) (i32.const 16)))
            (local.set $start (i32.add (local.get $at) (i32.const 1)))
            (local.set $m (i32.and (local.get $m) (i32.sub (local.get $m) (i32.const 1))))
            (br $bit)))
        (local.set $i (i32.add (local.get $i) (i32.const 16)))
        (br $next)))
    ;; The bytes left, and the last part, which ends at the end.
    (block $done
      (loop $next
        (if (i32.or (i32.ge_u (local.get $i) (local.get $end))
              (i32.eq (i32.load8_u (local.get $i)) (local.get $sep)))
          (then
            (i32.store (local.get $hdr) (i32.sub (local.get $i) (local.get $start)))
            (i32.store offset=4 (local.get $hdr) (i32.sub (local.get $i) (local.get $start)))
            (i32.store offset=8 (local.get $hdr) (local.get $start))
            (i32.store offset=12 (local.get $hdr) (i32.or (i32.const 1) (global.get $INLINE)))
            (i32.store (local.get $handle) (local.get $hdr))
            (local.set $handle (i32.add (local.get $handle) (i32.const 4)))
            (local.set $hdr (i32.add (local.get $hdr) (i32.const 16)))
            (br_if $done (i32.ge_u (local.get $i) (local.get $end)))
            (local.set $start (i32.add (local.get $i) (i32.const 1)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.get $parts))

  ;; @scalar
  ;; `str_split_views` on the `len` bytes at `data`, by the byte `sep`: the
  ;; separators counted in one loop, then the parts filled in another.
  (func $split_byte (param $data i32) (param $len i32) (param $sep i32) (result i32)
    (local $parts i32) (local $block i32) (local $hdr i32) (local $handle i32)
    (local $n i32) (local $i i32) (local $start i32) (local $end i32)
    (local.set $end (i32.add (local.get $data) (local.get $len)))
    (local.set $n (i32.const 1))
    (local.set $i (local.get $data))
    (block $counted
      (loop $next
        (br_if $counted (i32.ge_u (local.get $i) (local.get $end)))
        (if (i32.eq (i32.load8_u (local.get $i)) (local.get $sep))
          (then (local.set $n (i32.add (local.get $n) (i32.const 1)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.set $parts (call $alloc (i32.const 16) (i32.const 0)))
    (local.set $block
      (call $alloc (call $bytes (local.get $n) (i32.const 20) (i32.const 0)) (i32.const 0)))
    (i32.store (local.get $parts) (local.get $n))
    (i32.store offset=4 (local.get $parts) (local.get $n))
    (i32.store offset=8 (local.get $parts) (local.get $block))
    (i32.store offset=12 (local.get $parts) (i32.or (i32.const 4) (global.get $VIEWS)))
    ;; Handles at the start of the block, headers after them.
    (local.set $handle (local.get $block))
    (local.set $hdr (i32.add (local.get $block) (i32.shl (local.get $n) (i32.const 2))))
    (local.set $start (local.get $data))
    (local.set $i (local.get $data))
    (block $done
      (loop $next
        (if (i32.or (i32.ge_u (local.get $i) (local.get $end))
              (i32.eq (i32.load8_u (local.get $i)) (local.get $sep)))
          (then
            (i32.store (local.get $hdr) (i32.sub (local.get $i) (local.get $start)))
            (i32.store offset=4 (local.get $hdr) (i32.sub (local.get $i) (local.get $start)))
            (i32.store offset=8 (local.get $hdr) (local.get $start))
            (i32.store offset=12 (local.get $hdr) (i32.or (i32.const 1) (global.get $INLINE)))
            (i32.store (local.get $handle) (local.get $hdr))
            (local.set $handle (i32.add (local.get $handle) (i32.const 4)))
            (local.set $hdr (i32.add (local.get $hdr) (i32.const 16)))
            (br_if $done (i32.ge_u (local.get $i) (local.get $end)))
            (local.set $start (i32.add (local.get $i) (i32.const 1)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.get $parts))

  ;; @end

  ;; Part `c` of an array of views: a header for the `len` bytes at `at`,
  ;; and its handle.
  (func $view (param $block i32) (param $headers i32) (param $c i32) (param $at i32) (param $len i32)
    (local $hdr i32)
    (local.set $hdr (i32.add (local.get $headers) (i32.shl (local.get $c) (i32.const 4))))
    (i32.store (local.get $hdr) (local.get $len))
    (i32.store offset=4 (local.get $hdr) (local.get $len))
    (i32.store offset=8 (local.get $hdr) (local.get $at))
    (i32.store offset=12 (local.get $hdr) (i32.or (i32.const 1) (global.get $INLINE)))
    (i32.store (i32.add (local.get $block) (i32.shl (local.get $c) (i32.const 2))) (local.get $hdr)))

  ;; The strings of an array, with `separator` between each two.
  (func (export "arr_join") (param $parts i32) (param $separator i32) (result i32)
    (local $out i32) (local $i i32) (local $s i32)
    (local.set $out (call $buf_new (i32.const 0) (i32.const 1) (i32.const 0)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (i32.load (local.get $parts))))
        (if (local.get $i)
          (then (call $str_add (local.get $out) (i32.load offset=8 (local.get $separator))
            (i32.load (local.get $separator)))))
        (local.set $s (i32.load (i32.add (i32.load offset=8 (local.get $parts)) (i32.shl (local.get $i) (i32.const 2)))))
        (call $str_add (local.get $out) (i32.load offset=8 (local.get $s)) (i32.load (local.get $s)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.get $out))

  ;; Arrays, by column: a buffer of elements of `size` bytes, each column of
  ;; an array of structures on its own. A column of buffers (DEEP) owns them:
  ;; a new column gets copies.

  ;; The size of an element of `h`, in bytes.
  (func $element_size (param $h i32) (result i32)
    (i32.and (i32.load offset=12 (local.get $h)) (i32.const 0xFF)))

  ;; Each buffer of a new DEEP column replaced by a copy of it.
  (func $own_copies (param $h i32)
    (local $i i32) (local $at i32)
    (if (i32.eqz (i32.and (i32.load offset=12 (local.get $h)) (global.get $DEEP))) (then (return)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (i32.load (local.get $h))))
        (local.set $at (i32.add (i32.load offset=8 (local.get $h)) (i32.shl (local.get $i) (i32.const 2))))
        (i32.store (local.get $at) (call $buf_clone (i32.load (local.get $at)) (i32.const 0)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next))))

  ;; An index of an array made valid: negative from the end, then clamped.
  (func $clamp_index (param $len i32) (param $i i32) (result i32)
    (if (i32.lt_s (local.get $i) (i32.const 0))
      (then (local.set $i (i32.add (local.get $i) (local.get $len)))))
    (if (i32.lt_s (local.get $i) (i32.const 0)) (then (local.set $i (i32.const 0))))
    (if (i32.gt_s (local.get $i) (local.get $len)) (then (local.set $i (local.get $len))))
    (local.get $i))

  ;; A new column with the elements from `start` to `end`.
  (func (export "arr_slice") (param $h i32) (param $start i32) (param $end i32) (result i32)
    (local $len i32) (local $es i32) (local $n i32) (local $out i32)
    (local.set $len (i32.load (local.get $h)))
    (local.set $es (call $element_size (local.get $h)))
    (local.set $start (call $clamp_index (local.get $len) (local.get $start)))
    (local.set $end (call $clamp_index (local.get $len) (local.get $end)))
    (if (i32.lt_s (local.get $end) (local.get $start)) (then (local.set $end (local.get $start))))
    (local.set $n (i32.sub (local.get $end) (local.get $start)))
    (local.set $out (call $buf_new (local.get $n) (i32.load offset=12 (local.get $h)) (i32.const 0)))
    (if (local.get $n)
      (then (memory.copy (i32.load offset=8 (local.get $out))
        (i32.add (i32.load offset=8 (local.get $h)) (i32.mul (local.get $start) (local.get $es)))
        (i32.mul (local.get $n) (local.get $es)))))
    (i32.store (local.get $out) (local.get $n))
    (call $own_copies (local.get $out))
    (local.get $out))

  ;; A new column with the elements of `a`, then those of `b`.
  (func (export "arr_concat") (param $a i32) (param $b i32) (result i32)
    (local $la i32) (local $lb i32) (local $es i32) (local $out i32)
    (local.set $la (i32.load (local.get $a)))
    (local.set $lb (i32.load (local.get $b)))
    (local.set $es (call $element_size (local.get $a)))
    (local.set $out (call $buf_new (i32.add (local.get $la) (local.get $lb))
      (i32.load offset=12 (local.get $a)) (i32.const 0)))
    (if (local.get $la)
      (then (memory.copy (i32.load offset=8 (local.get $out)) (i32.load offset=8 (local.get $a))
        (i32.mul (local.get $la) (local.get $es)))))
    (if (local.get $lb)
      (then (memory.copy
        (i32.add (i32.load offset=8 (local.get $out)) (i32.mul (local.get $la) (local.get $es)))
        (i32.load offset=8 (local.get $b)) (i32.mul (local.get $lb) (local.get $es)))))
    (i32.store (local.get $out) (i32.add (local.get $la) (local.get $lb)))
    (call $own_copies (local.get $out))
    (local.get $out))

  ;; Swaps the elements at `i` and `j` of `h`, of `es` bytes: 1, 4 or 8.
  (func $swap (param $h i32) (param $i i32) (param $j i32) (param $es i32)
    (local $a i32) (local $b i32) (local $t i64)
    (local.set $a (i32.add (i32.load offset=8 (local.get $h)) (i32.mul (local.get $i) (local.get $es))))
    (local.set $b (i32.add (i32.load offset=8 (local.get $h)) (i32.mul (local.get $j) (local.get $es))))
    (if (i32.eq (local.get $es) (i32.const 8))
      (then
        (local.set $t (i64.load (local.get $a)))
        (i64.store (local.get $a) (i64.load (local.get $b)))
        (i64.store (local.get $b) (local.get $t))
        (return)))
    (if (i32.eq (local.get $es) (i32.const 4))
      (then
        (local.set $t (i64.extend_i32_u (i32.load (local.get $a))))
        (i32.store (local.get $a) (i32.load (local.get $b)))
        (i32.store (local.get $b) (i32.wrap_i64 (local.get $t)))
        (return)))
    (local.set $t (i64.extend_i32_u (i32.load8_u (local.get $a))))
    (i32.store8 (local.get $a) (i32.load8_u (local.get $b)))
    (i32.store8 (local.get $b) (i32.wrap_i64 (local.get $t))))

  ;; Keeps, in place and in order, the elements of `h` whose flag in `flags`,
  ;; an array of booleans as long, is set; releases the buffers among the
  ;; others when `h` owns them; shortens `h` to what it kept.
  (func (export "arr_keep") (param $h i32) (param $flags i32)
    (local $n i32) (local $i i32) (local $w i32) (local $es i32) (local $data i32)
    (local $f i32) (local $deep i32)
    (local.set $n (i32.load (local.get $h)))
    (local.set $es (call $element_size (local.get $h)))
    (local.set $deep (i32.and (i32.load offset=12 (local.get $h)) (global.get $DEEP)))
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $f (i32.load offset=8 (local.get $flags)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
        (if (i32.load (i32.add (local.get $f) (i32.shl (local.get $i) (i32.const 2))))
          (then
            (if (i32.ne (local.get $w) (local.get $i))
              (then (memory.copy
                (i32.add (local.get $data) (i32.mul (local.get $w) (local.get $es)))
                (i32.add (local.get $data) (i32.mul (local.get $i) (local.get $es)))
                (local.get $es))))
            (local.set $w (i32.add (local.get $w) (i32.const 1))))
          (else
            (if (local.get $deep)
              (then (call $buf_release
                (i32.load (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 2))))
                (i32.const 0))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.store (local.get $h) (local.get $w)))

  ;; Reverses the elements of `h` in place.
  (func (export "arr_reverse") (param $h i32)
    (local $i i32) (local $j i32) (local $es i32)
    (local.set $es (call $element_size (local.get $h)))
    (local.set $j (i32.sub (i32.load (local.get $h)) (i32.const 1)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_s (local.get $i) (local.get $j)))
        (call $swap (local.get $h) (local.get $i) (local.get $j) (local.get $es))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $j (i32.sub (local.get $j) (i32.const 1)))
        (br $next))))

  ;; Sorting. Each number becomes a key, an unsigned integer in the same
  ;; order; the keys are sorted, then turned back into numbers. The order is
  ;; JavaScript's for typed arrays: -0 before +0, and every NaN last, whatever
  ;; its sign; a NaN comes back as the canonical one, as WebAssembly does not
  ;; fix the bits of a NaN either.

  ;; The key of a 32-bit number: the sign bit flipped for an integer; for a
  ;; float, every bit flipped when it is negative, else the sign bit, and the
  ;; largest key for a NaN.
  (func $key32 (param $x i32) (param $float i32) (result i32)
    (if (i32.eqz (local.get $float))
      (then (return (i32.xor (local.get $x) (i32.const 0x80000000)))))
    (if (i32.gt_u (i32.and (local.get $x) (i32.const 0x7fffffff)) (i32.const 0x7f800000))
      (then (return (i32.const -1))))
    (if (result i32) (i32.lt_s (local.get $x) (i32.const 0))
      (then (i32.xor (local.get $x) (i32.const -1)))
      (else (i32.xor (local.get $x) (i32.const 0x80000000)))))

  (func $unkey32 (param $k i32) (param $float i32) (result i32)
    (if (i32.eqz (local.get $float))
      (then (return (i32.xor (local.get $k) (i32.const 0x80000000)))))
    (if (i32.eq (local.get $k) (i32.const -1))
      (then (return (i32.const 0x7fc00000))))
    ;; The sign bit set: the number was positive.
    (if (result i32) (i32.lt_s (local.get $k) (i32.const 0))
      (then (i32.xor (local.get $k) (i32.const 0x80000000)))
      (else (i32.xor (local.get $k) (i32.const -1)))))

  (func $key64 (param $x i64) (param $float i32) (result i64)
    (if (i32.eqz (local.get $float))
      (then (return (i64.xor (local.get $x) (i64.const 0x8000000000000000)))))
    (if (i64.gt_u (i64.and (local.get $x) (i64.const 0x7fffffffffffffff))
          (i64.const 0x7ff0000000000000))
      (then (return (i64.const -1))))
    (if (result i64) (i64.lt_s (local.get $x) (i64.const 0))
      (then (i64.xor (local.get $x) (i64.const -1)))
      (else (i64.xor (local.get $x) (i64.const 0x8000000000000000)))))

  (func $unkey64 (param $k i64) (param $float i32) (result i64)
    (if (i32.eqz (local.get $float))
      (then (return (i64.xor (local.get $k) (i64.const 0x8000000000000000)))))
    (if (i64.eq (local.get $k) (i64.const -1))
      (then (return (i64.const 0x7ff8000000000000))))
    (if (result i64) (i64.lt_s (local.get $k) (i64.const 0))
      (then (i64.xor (local.get $k) (i64.const 0x8000000000000000)))
      (else (i64.xor (local.get $k) (i64.const -1)))))

  ;; Insertion sort of the `n` keys at `data`: for a few keys, where counting
  ;; costs more than it saves.
  (func $insert32 (param $data i32) (param $n i32)
    (local $i i32) (local $at i32) (local $k i32) (local $prev i32)
    (local.set $i (i32.const 1))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
        (local.set $at (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 2))))
        (local.set $k (i32.load (local.get $at)))
        (block $placed
          (loop $shift
            (br_if $placed (i32.eq (local.get $at) (local.get $data)))
            (local.set $prev (i32.load (i32.sub (local.get $at) (i32.const 4))))
            (br_if $placed (i32.le_u (local.get $prev) (local.get $k)))
            (i32.store (local.get $at) (local.get $prev))
            (local.set $at (i32.sub (local.get $at) (i32.const 4)))
            (br $shift)))
        (i32.store (local.get $at) (local.get $k))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next))))

  (func $insert64 (param $data i32) (param $n i32)
    (local $i i32) (local $at i32) (local $k i64) (local $prev i64)
    (local.set $i (i32.const 1))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
        (local.set $at (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 3))))
        (local.set $k (i64.load (local.get $at)))
        (block $placed
          (loop $shift
            (br_if $placed (i32.eq (local.get $at) (local.get $data)))
            (local.set $prev (i64.load (i32.sub (local.get $at) (i32.const 8))))
            (br_if $placed (i64.le_u (local.get $prev) (local.get $k)))
            (i64.store (local.get $at) (local.get $prev))
            (local.set $at (i32.sub (local.get $at) (i32.const 8)))
            (br $shift)))
        (i64.store (local.get $at) (local.get $k))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next))))

  ;; LSD radix sort of the `n` keys at `data`, 3 digits of 11 bits, through a
  ;; scratch block as large, plus 3 tables of 2048 counts. A digit that is
  ;; the same in every key, as the high bits of small numbers, is skipped.
  (func $radix32 (param $data i32) (param $n i32)
    (local $block i32) (local $counts i32) (local $src i32) (local $dst i32)
    (local $i i32) (local $d i32) (local $b i32) (local $at i32) (local $sum i32) (local $c i32)
    (local $shift i32) (local $k i32) (local $bytes i32)
    (local.set $bytes (call $bytes (local.get $n) (i32.const 4) (i32.const 0)))
    (local.set $block (call $alloc
      (i32.add (local.get $bytes) (i32.const 24576)) (i32.const 0)))
    (local.set $counts (i32.add (local.get $block) (local.get $bytes)))
    (memory.fill (local.get $counts) (i32.const 0) (i32.const 24576))
    ;; Every digit's counts, in one reading of the keys.
    (local.set $i (i32.const 0))
    (block $counted
      (loop $next
        (br_if $counted (i32.ge_u (local.get $i) (local.get $n)))
        (local.set $k (i32.load (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 2)))))
        (local.set $d (i32.const 0))
        (block $digits
          (loop $digit
            (br_if $digits (i32.ge_u (local.get $d) (i32.const 3)))
            (local.set $at (i32.add
              (i32.add (local.get $counts) (i32.shl (local.get $d) (i32.const 13)))
              (i32.shl (i32.and (i32.shr_u (local.get $k) (i32.mul (local.get $d) (i32.const 11))) (i32.const 2047)) (i32.const 2))))
            (i32.store (local.get $at) (i32.add (i32.load (local.get $at)) (i32.const 1)))
            (local.set $d (i32.add (local.get $d) (i32.const 1)))
            (br $digit)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.set $src (local.get $data))
    (local.set $dst (local.get $block))
    (local.set $d (i32.const 0))
    (block $passes
      (loop $pass
        (br_if $passes (i32.ge_u (local.get $d) (i32.const 3)))
        (local.set $shift (i32.mul (local.get $d) (i32.const 11)))
        (local.set $counts (i32.add (local.get $block)
          (i32.add (local.get $bytes) (i32.shl (local.get $d) (i32.const 13)))))
        ;; Skipped when one bucket holds every key.
        (if (i32.ne (local.get $n)
              (i32.load (i32.add (local.get $counts)
                (i32.shl (i32.and (i32.shr_u (i32.load (local.get $src)) (local.get $shift)) (i32.const 2047)) (i32.const 2)))))
          (then
            ;; Where each bucket starts.
            (local.set $sum (i32.const 0))
            (local.set $b (i32.const 0))
            (block $starts
              (loop $start
                (br_if $starts (i32.ge_u (local.get $b) (i32.const 2048)))
                (local.set $at (i32.add (local.get $counts) (i32.shl (local.get $b) (i32.const 2))))
                (local.set $c (i32.load (local.get $at)))
                (i32.store (local.get $at) (local.get $sum))
                (local.set $sum (i32.add (local.get $sum) (local.get $c)))
                (local.set $b (i32.add (local.get $b) (i32.const 1)))
                (br $start)))
            ;; Each key to its bucket, in order: the sort is stable.
            (local.set $i (i32.const 0))
            (block $moved
              (loop $move
                (br_if $moved (i32.ge_u (local.get $i) (local.get $n)))
                (local.set $k (i32.load (i32.add (local.get $src) (i32.shl (local.get $i) (i32.const 2)))))
                (local.set $at (i32.add (local.get $counts)
                  (i32.shl (i32.and (i32.shr_u (local.get $k) (local.get $shift)) (i32.const 2047)) (i32.const 2))))
                (local.set $c (i32.load (local.get $at)))
                (i32.store (local.get $at) (i32.add (local.get $c) (i32.const 1)))
                (i32.store (i32.add (local.get $dst) (i32.shl (local.get $c) (i32.const 2))) (local.get $k))
                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                (br $move)))
            (local.set $c (local.get $src))
            (local.set $src (local.get $dst))
            (local.set $dst (local.get $c))))
        (local.set $d (i32.add (local.get $d) (i32.const 1)))
        (br $pass)))
    (if (i32.ne (local.get $src) (local.get $data))
      (then (memory.copy (local.get $data) (local.get $src) (local.get $bytes))))
    (call $release (local.get $block) (i32.const 0)))

  ;; LSD radix sort of the `n` keys at `data`, 6 digits of 11 bits, through a
  ;; scratch block as large, plus 6 tables of 2048 counts. A digit that is
  ;; the same in every key, as the high bits of small numbers, is skipped.
  (func $radix64 (param $data i32) (param $n i32)
    (local $block i32) (local $counts i32) (local $src i32) (local $dst i32)
    (local $i i32) (local $d i32) (local $b i32) (local $at i32) (local $sum i32) (local $c i32)
    (local $shift i32) (local $k i64) (local $bytes i32)
    (local.set $bytes (call $bytes (local.get $n) (i32.const 8) (i32.const 0)))
    (local.set $block (call $alloc
      (i32.add (local.get $bytes) (i32.const 49152)) (i32.const 0)))
    (local.set $counts (i32.add (local.get $block) (local.get $bytes)))
    (memory.fill (local.get $counts) (i32.const 0) (i32.const 49152))
    ;; Every digit's counts, in one reading of the keys.
    (local.set $i (i32.const 0))
    (block $counted
      (loop $next
        (br_if $counted (i32.ge_u (local.get $i) (local.get $n)))
        (local.set $k (i64.load (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 3)))))
        (local.set $d (i32.const 0))
        (block $digits
          (loop $digit
            (br_if $digits (i32.ge_u (local.get $d) (i32.const 6)))
            (local.set $at (i32.add
              (i32.add (local.get $counts) (i32.shl (local.get $d) (i32.const 13)))
              (i32.shl (i32.wrap_i64 (i64.and (i64.shr_u (local.get $k) (i64.extend_i32_u (i32.mul (local.get $d) (i32.const 11)))) (i64.const 2047))) (i32.const 2))))
            (i32.store (local.get $at) (i32.add (i32.load (local.get $at)) (i32.const 1)))
            (local.set $d (i32.add (local.get $d) (i32.const 1)))
            (br $digit)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.set $src (local.get $data))
    (local.set $dst (local.get $block))
    (local.set $d (i32.const 0))
    (block $passes
      (loop $pass
        (br_if $passes (i32.ge_u (local.get $d) (i32.const 6)))
        (local.set $shift (i32.mul (local.get $d) (i32.const 11)))
        (local.set $counts (i32.add (local.get $block)
          (i32.add (local.get $bytes) (i32.shl (local.get $d) (i32.const 13)))))
        ;; Skipped when one bucket holds every key.
        (if (i32.ne (local.get $n)
              (i32.load (i32.add (local.get $counts)
                (i32.shl (i32.wrap_i64 (i64.and (i64.shr_u (i64.load (local.get $src)) (i64.extend_i32_u (local.get $shift))) (i64.const 2047))) (i32.const 2)))))
          (then
            ;; Where each bucket starts.
            (local.set $sum (i32.const 0))
            (local.set $b (i32.const 0))
            (block $starts
              (loop $start
                (br_if $starts (i32.ge_u (local.get $b) (i32.const 2048)))
                (local.set $at (i32.add (local.get $counts) (i32.shl (local.get $b) (i32.const 2))))
                (local.set $c (i32.load (local.get $at)))
                (i32.store (local.get $at) (local.get $sum))
                (local.set $sum (i32.add (local.get $sum) (local.get $c)))
                (local.set $b (i32.add (local.get $b) (i32.const 1)))
                (br $start)))
            ;; Each key to its bucket, in order: the sort is stable.
            (local.set $i (i32.const 0))
            (block $moved
              (loop $move
                (br_if $moved (i32.ge_u (local.get $i) (local.get $n)))
                (local.set $k (i64.load (i32.add (local.get $src) (i32.shl (local.get $i) (i32.const 3)))))
                (local.set $at (i32.add (local.get $counts)
                  (i32.shl (i32.wrap_i64 (i64.and (i64.shr_u (local.get $k) (i64.extend_i32_u (local.get $shift))) (i64.const 2047))) (i32.const 2))))
                (local.set $c (i32.load (local.get $at)))
                (i32.store (local.get $at) (i32.add (local.get $c) (i32.const 1)))
                (i64.store (i32.add (local.get $dst) (i32.shl (local.get $c) (i32.const 3))) (local.get $k))
                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                (br $move)))
            (local.set $c (local.get $src))
            (local.set $src (local.get $dst))
            (local.set $dst (local.get $c))))
        (local.set $d (i32.add (local.get $d) (i32.const 1)))
        (br $pass)))
    (if (i32.ne (local.get $src) (local.get $data))
      (then (memory.copy (local.get $data) (local.get $src) (local.get $bytes))))
    (call $release (local.get $block) (i32.const 0)))

  ;; Sorts the numbers of `h` in increasing order, in place: `kind` is 0 for
  ;; `i32`, 1 `i64`, 2 `f32`, 3 `f64`. Below 64 numbers, an insertion sort;
  ;; above, a radix sort in a constant number of passes, time n.
  (func (export "arr_sort") (param $h i32) (param $kind i32)
    (local $n i32) (local $data i32) (local $float i32) (local $i i32) (local $at i32)
    (local.set $n (i32.load (local.get $h)))
    (if (i32.lt_u (local.get $n) (i32.const 2)) (then (return)))
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $float (i32.ge_u (local.get $kind) (i32.const 2)))
    (if (i32.and (local.get $kind) (i32.const 1))
      (then
        (local.set $i (i32.const 0))
        (block $keyed
          (loop $next
            (br_if $keyed (i32.ge_u (local.get $i) (local.get $n)))
            (local.set $at (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 3))))
            (i64.store (local.get $at) (call $key64 (i64.load (local.get $at)) (local.get $float)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next)))
        (if (i32.lt_u (local.get $n) (i32.const 64))
          (then (call $insert64 (local.get $data) (local.get $n)))
          (else (call $radix64 (local.get $data) (local.get $n))))
        (local.set $i (i32.const 0))
        (block $back
          (loop $next
            (br_if $back (i32.ge_u (local.get $i) (local.get $n)))
            (local.set $at (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 3))))
            (i64.store (local.get $at) (call $unkey64 (i64.load (local.get $at)) (local.get $float)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next))))
      (else
        (local.set $i (i32.const 0))
        (block $keyed
          (loop $next
            (br_if $keyed (i32.ge_u (local.get $i) (local.get $n)))
            (local.set $at (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 2))))
            (i32.store (local.get $at) (call $key32 (i32.load (local.get $at)) (local.get $float)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next)))
        (if (i32.lt_u (local.get $n) (i32.const 64))
          (then (call $insert32 (local.get $data) (local.get $n)))
          (else (call $radix32 (local.get $data) (local.get $n))))
        (local.set $i (i32.const 0))
        (block $back
          (loop $next
            (br_if $back (i32.ge_u (local.get $i) (local.get $n)))
            (local.set $at (i32.add (local.get $data) (i32.shl (local.get $i) (i32.const 2))))
            (i32.store (local.get $at) (call $unkey32 (i32.load (local.get $at)) (local.get $float)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next))))))

  ;; The index of the first string of `h` equal to `s`, or -1.
  (func (export "arr_index_of_str") (param $h i32) (param $s i32) (result i32)
    (local $i i32) (local $e i32)
    (block $none
      (loop $next
        (br_if $none (i32.ge_u (local.get $i) (i32.load (local.get $h))))
        (local.set $e (i32.load (i32.add (i32.load offset=8 (local.get $h)) (i32.shl (local.get $i) (i32.const 2)))))
        (if (i32.eq (i32.load (local.get $e)) (i32.load (local.get $s)))
          (then (if (call $same (i32.load offset=8 (local.get $e)) (i32.load offset=8 (local.get $s)) (i32.load (local.get $s)))
            (then (return (local.get $i))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.const -1))

  ;; Numbers and booleans as text, and text as integers.

  ;; The decimal digits of `x`, after `-` when it is negative: written from
  ;; the end of a 20-byte block, the longest an `i64` needs, then moved to
  ;; its start.
  (func (export "fmt_int") (param $x i64) (result i32)
    (local $h i32) (local $p i32) (local $end i32) (local $neg i32) (local $n i32)
    (local.set $h (call $buf_new (i32.const 20) (i32.const 1) (i32.const 0)))
    (local.set $end (i32.add (i32.load offset=8 (local.get $h)) (i32.const 20)))
    (local.set $p (local.get $end))
    ;; The magnitude, unsigned: right for the smallest `i64` too.
    (if (i64.lt_s (local.get $x) (i64.const 0))
      (then
        (local.set $neg (i32.const 1))
        (local.set $x (i64.sub (i64.const 0) (local.get $x)))))
    (loop $digit
      (local.set $p (i32.sub (local.get $p) (i32.const 1)))
      (i32.store8 (local.get $p)
        (i32.add (i32.const 48) (i32.wrap_i64 (i64.rem_u (local.get $x) (i64.const 10)))))
      (local.set $x (i64.div_u (local.get $x) (i64.const 10)))
      (br_if $digit (i64.ne (local.get $x) (i64.const 0))))
    (if (local.get $neg)
      (then
        (local.set $p (i32.sub (local.get $p) (i32.const 1)))
        (i32.store8 (local.get $p) (i32.const 45))))
    (local.set $n (i32.sub (local.get $end) (local.get $p)))
    (memory.copy (i32.load offset=8 (local.get $h)) (local.get $p) (local.get $n))
    (i32.store (local.get $h) (local.get $n))
    (local.get $h))

  ;; `true` or `false`, their bytes stored as one word, little-endian.
  (func (export "fmt_bool") (param $b i32) (result i32)
    (local $h i32) (local $at i32)
    (local.set $h (call $buf_new (i32.const 5) (i32.const 1) (i32.const 0)))
    (local.set $at (i32.load offset=8 (local.get $h)))
    (if (local.get $b)
      (then
        (i32.store (local.get $at) (i32.const 0x65757274))
        (i32.store (local.get $h) (i32.const 4)))
      (else
        (i32.store (local.get $at) (i32.const 0x736c6166))
        (i32.store8 offset=4 (local.get $at) (i32.const 0x65))
        (i32.store (local.get $h) (i32.const 5))))
    (local.get $h))

  ;; `x` as text, written by the host into a new string: `-1.2345678901234567e-308`,
  ;; 24 bytes, and `-0.0000012345678901234567`, 25, are the longest.
  (func (export "fmt_float") (param $x f64) (param $single i32) (result i32)
    (local $h i32)
    (local.set $h (call $buf_new (i32.const 32) (i32.const 1) (i32.const 0)))
    (i32.store (local.get $h)
      (call $float_text (local.get $x) (local.get $single) (i32.load offset=8 (local.get $h))))
    (local.get $h))

    ;; Where the digits of `h` start: after a sign, if any. `$negative` says
  ;; which sign.
  (global $negative (mut i32) (i32.const 0))
  (func $digits_from (param $h i32) (result i32)
    (local $c i32)
    (global.set $negative (i32.const 0))
    (if (i32.eqz (i32.load (local.get $h))) (then (return (i32.const 0))))
    (local.set $c (i32.load8_u (i32.load offset=8 (local.get $h))))
    (global.set $negative (i32.eq (local.get $c) (i32.const 45)))
    (i32.or (global.get $negative) (i32.eq (local.get $c) (i32.const 43))))

  ;; Whether `h` is a sign or none, then at least one decimal digit, and
  ;; nothing else, in the range of a signed integer of `bits` bits: up to
  ;; 2^(bits-1) - 1, or 2^(bits-1) after `-`.
  (func (export "str_is_int") (param $h i32) (param $bits i32) (result i32)
    (local $len i32) (local $at i32) (local $i i32) (local $c i32)
    (local $limit i64) (local $mag i64) (local $d i64)
    (local.set $len (i32.load (local.get $h)))
    (local.set $at (i32.load offset=8 (local.get $h)))
    (local.set $i (call $digits_from (local.get $h)))
    (if (i32.ge_u (local.get $i) (local.get $len)) (then (return (i32.const 0))))
    (local.set $limit (i64.add
      (i64.sub
        (i64.shl (i64.const 1) (i64.extend_i32_u (i32.sub (local.get $bits) (i32.const 1))))
        (i64.const 1))
      (i64.extend_i32_u (global.get $negative))))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $c (i32.sub
          (i32.load8_u (i32.add (local.get $at) (local.get $i))) (i32.const 48)))
        (if (i32.gt_u (local.get $c) (i32.const 9)) (then (return (i32.const 0))))
        (local.set $d (i64.extend_i32_u (local.get $c)))
        ;; `mag * 10 + d` above the limit is refused before it is computed.
        (if (i64.gt_u (local.get $mag)
              (i64.div_u (i64.sub (local.get $limit) (local.get $d)) (i64.const 10)))
          (then (return (i32.const 0))))
        (local.set $mag (i64.add (i64.mul (local.get $mag) (i64.const 10)) (local.get $d)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.const 1))

  ;; The integer `h` writes, which `str_is_int` accepted.
  (func (export "str_parse_int") (param $h i32) (result i64)
    (local $len i32) (local $at i32) (local $i i32) (local $mag i64)
    (local.set $len (i32.load (local.get $h)))
    (local.set $at (i32.load offset=8 (local.get $h)))
    (local.set $i (call $digits_from (local.get $h)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $mag (i64.add
          (i64.mul (local.get $mag) (i64.const 10))
          (i64.extend_i32_u (i32.sub
            (i32.load8_u (i32.add (local.get $at) (local.get $i))) (i32.const 48)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (select (i64.sub (i64.const 0) (local.get $mag)) (local.get $mag) (global.get $negative)))

  ;; Maps and sets (decision 0005): in insertion order, CPython's compact
  ;; layout, hashed with SipHash-1-3 under a secret key. A map is a 32-byte
  ;; header; `size` reads its +0, as an array's length:
  ;;
  ;;   +0 live entries   +4 entries used, removed ones included   +8 the entries
  ;;   +12 MAP, plus STRKEY when its keys are strings it owns   +16 room for entries
  ;;   +20 the index   +24 its slots minus one, a power of two minus one
  ;;   +28 its shape: the number of value slots in the low byte, and above it
  ;;       one bit per slot that holds a buffer the map owns
  ;;
  ;; An entry is 16 bytes and 8 per value slot, in insertion order: +0 the
  ;; low 32 bits of its key's hash, +4 1 while it is live, +8 its key, an
  ;; integer as an i64 or a string's handle, +16 its value slots, the scalars
  ;; of the value, each in 64 bits. A set has none. An index slot holds 0 when
  ;; empty, -1 where an entry was removed, otherwise the entry's number plus
  ;; one. The index is at most two thirds full, removed slots included: it is
  ;; rebuilt, and the entries packed, when the entries are full.
  (global $MAP i32 (i32.const 0x800))
  (global $STRKEY i32 (i32.const 0x1000))
  ;; SipHash-1-3's key, set once by the loader with `map_seed` from
  ;; crypto.getRandomValues; fixed until then, for tests.
  (global $k0 (mut i64) (i64.const 0x0706050403020100))
  (global $k1 (mut i64) (i64.const 0x0f0e0d0c0b0a0908))
  (global $v0 (mut i64) (i64.const 0))
  (global $v1 (mut i64) (i64.const 0))
  (global $v2 (mut i64) (i64.const 0))
  (global $v3 (mut i64) (i64.const 0))

  (func (export "map_seed") (param $a i64) (param $b i64)
    (global.set $k0 (local.get $a))
    (global.set $k1 (local.get $b)))

  ;; One SipRound on v0..v3.
  (func $sip_round
    (global.set $v0 (i64.add (global.get $v0) (global.get $v1)))
    (global.set $v1 (i64.xor (i64.rotl (global.get $v1) (i64.const 13)) (global.get $v0)))
    (global.set $v0 (i64.rotl (global.get $v0) (i64.const 32)))
    (global.set $v2 (i64.add (global.get $v2) (global.get $v3)))
    (global.set $v3 (i64.xor (i64.rotl (global.get $v3) (i64.const 16)) (global.get $v2)))
    (global.set $v0 (i64.add (global.get $v0) (global.get $v3)))
    (global.set $v3 (i64.xor (i64.rotl (global.get $v3) (i64.const 21)) (global.get $v0)))
    (global.set $v2 (i64.add (global.get $v2) (global.get $v1)))
    (global.set $v1 (i64.xor (i64.rotl (global.get $v1) (i64.const 17)) (global.get $v2)))
    (global.set $v2 (i64.rotl (global.get $v2) (i64.const 32))))

  (func $sip_init
    (global.set $v0 (i64.xor (global.get $k0) (i64.const 0x736f6d6570736575)))
    (global.set $v1 (i64.xor (global.get $k1) (i64.const 0x646f72616e646f6d)))
    (global.set $v2 (i64.xor (global.get $k0) (i64.const 0x6c7967656e657261)))
    (global.set $v3 (i64.xor (global.get $k1) (i64.const 0x7465646279746573))))

  ;; One message word: one compression round, SipHash-1-3's "1".
  (func $sip_word (param $m i64)
    (global.set $v3 (i64.xor (global.get $v3) (local.get $m)))
    (call $sip_round)
    (global.set $v0 (i64.xor (global.get $v0) (local.get $m))))

  ;; Three finalization rounds, the "3".
  (func $sip_end (result i64)
    (global.set $v2 (i64.xor (global.get $v2) (i64.const 0xff)))
    (call $sip_round)
    (call $sip_round)
    (call $sip_round)
    (i64.xor (i64.xor (global.get $v0) (global.get $v1)) (i64.xor (global.get $v2) (global.get $v3))))

  ;; SipHash-1-3 of `len` bytes at `at`.
  (func $sip_bytes (export "map_sip") (param $at i32) (param $len i32) (result i64)
    (local $i i32) (local $last i64) (local $j i32)
    (call $sip_init)
    (block $done
      (loop $next
        (br_if $done (i32.gt_u (i32.add (local.get $i) (i32.const 8)) (local.get $len)))
        (call $sip_word (i64.load (i32.add (local.get $at) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 8)))
        (br $next)))
    (local.set $last (i64.shl (i64.extend_i32_u (local.get $len)) (i64.const 56)))
    (block $tail
      (loop $byte
        (br_if $tail (i32.ge_u (i32.add (local.get $i) (local.get $j)) (local.get $len)))
        (local.set $last (i64.or (local.get $last)
          (i64.shl
            (i64.extend_i32_u (i32.load8_u (i32.add (local.get $at) (i32.add (local.get $i) (local.get $j)))))
            (i64.extend_i32_u (i32.shl (local.get $j) (i32.const 3))))))
        (local.set $j (i32.add (local.get $j) (i32.const 1)))
        (br $byte)))
    (call $sip_word (local.get $last))
    (call $sip_end))

  ;; SipHash-1-3 of an integer key's 8 bytes, little-endian: the same as
  ;; `$sip_bytes` over them.
  (func $sip_int (param $x i64) (result i64)
    (call $sip_init)
    (call $sip_word (local.get $x))
    (call $sip_word (i64.const 0x0800000000000000))
    (call $sip_end))

  (func $map_strings (param $m i32) (result i32)
    (i32.ne (i32.and (i32.load offset=12 (local.get $m)) (global.get $STRKEY)) (i32.const 0)))

  (func $map_hash (param $m i32) (param $key i64) (result i32)
    (local $s i32)
    (if (result i32) (call $map_strings (local.get $m))
      (then
        (local.set $s (i32.wrap_i64 (local.get $key)))
        (i32.wrap_i64 (call $sip_bytes (i32.load offset=8 (local.get $s)) (i32.load (local.get $s)))))
      (else (i32.wrap_i64 (call $sip_int (local.get $key))))))

  (func $map_same (param $m i32) (param $a i64) (param $b i64) (result i32)
    (local $x i32) (local $y i32)
    (if (result i32) (call $map_strings (local.get $m))
      (then
        (local.set $x (i32.wrap_i64 (local.get $a)))
        (local.set $y (i32.wrap_i64 (local.get $b)))
        (if (result i32) (i32.ne (i32.load (local.get $x)) (i32.load (local.get $y)))
          (then (i32.const 0))
          (else (call $same (i32.load offset=8 (local.get $x)) (i32.load offset=8 (local.get $y))
            (i32.load (local.get $x))))))
      (else (i64.eq (local.get $a) (local.get $b)))))

  ;; The address of the index slot that holds `key`, or 0.
  (func $map_slot (param $m i32) (param $hash i32) (param $key i64) (result i32)
    (local $index i32) (local $mask i32) (local $s i32) (local $slot i32) (local $at i32)
    (local.set $index (i32.load offset=20 (local.get $m)))
    (if (i32.eqz (local.get $index)) (then (return (i32.const 0))))
    (local.set $mask (i32.load offset=24 (local.get $m)))
    (local.set $s (i32.and (local.get $hash) (local.get $mask)))
    (loop $probe
      (local.set $at (i32.add (local.get $index) (i32.shl (local.get $s) (i32.const 2))))
      (local.set $slot (i32.load (local.get $at)))
      (if (i32.eqz (local.get $slot)) (then (return (i32.const 0))))
      (if (i32.gt_s (local.get $slot) (i32.const 0))
        (then
          (local.set $slot (call $map_entry (local.get $m) (i32.sub (local.get $slot) (i32.const 1))))
          (if (i32.and
                (i32.eq (i32.load (local.get $slot)) (local.get $hash))
                (call $map_same (local.get $m) (i64.load offset=8 (local.get $slot)) (local.get $key)))
            (then (return (local.get $at))))))
      (local.set $s (i32.and (i32.add (local.get $s) (i32.const 1)) (local.get $mask)))
      (br $probe))
    (i32.const 0))

  ;; The size of an entry of a map of this shape.
  (func $map_stride (param $shape i32) (result i32)
    (i32.add (i32.const 16) (i32.shl (i32.and (local.get $shape) (i32.const 0xff)) (i32.const 3))))

  ;; The address of entry `e`.
  (func $map_entry (param $m i32) (param $e i32) (result i32)
    (i32.add (i32.load offset=8 (local.get $m))
      (i32.mul (local.get $e) (call $map_stride (i32.load offset=28 (local.get $m))))))

  ;; Releases the buffers the value slots of `entry` own.
  (func $map_release_values (param $shape i32) (param $entry i32)
    (local $owned i32) (local $at i32)
    (local.set $owned (i32.shr_u (local.get $shape) (i32.const 8)))
    (local.set $at (i32.add (local.get $entry) (i32.const 16)))
    (block $done
      (loop $next
        (br_if $done (i32.eqz (local.get $owned)))
        (if (i32.and (local.get $owned) (i32.const 1))
          (then (call $buf_release (i32.wrap_i64 (i64.load (local.get $at))) (i32.const 0))))
        (local.set $owned (i32.shr_u (local.get $owned) (i32.const 1)))
        (local.set $at (i32.add (local.get $at) (i32.const 8)))
        (br $next))))

  ;; Puts entry `e` into the index, in the first free slot from its hash.
  (func $map_place (param $m i32) (param $e i32)
    (local $mask i32) (local $s i32) (local $at i32)
    (local.set $mask (i32.load offset=24 (local.get $m)))
    (local.set $s (i32.and (i32.load (call $map_entry (local.get $m) (local.get $e))) (local.get $mask)))
    (loop $probe
      (local.set $at (i32.add (i32.load offset=20 (local.get $m)) (i32.shl (local.get $s) (i32.const 2))))
      (if (i32.gt_s (i32.load (local.get $at)) (i32.const 0))
        (then
          (local.set $s (i32.and (i32.add (local.get $s) (i32.const 1)) (local.get $mask)))
          (br $probe))))
    (i32.store (local.get $at) (i32.add (local.get $e) (i32.const 1))))

  ;; Room for twice the live entries, at least 4: the live ones packed, in
  ;; order, and a new index, at most two thirds full.
  (func $map_grow (param $m i32)
    (local $live i32) (local $cap i32) (local $slots i32) (local $old i32) (local $used i32)
    (local $entries i32) (local $index i32) (local $i i32) (local $n i32) (local $from i32)
    (local $stride i32)
    (local.set $stride (call $map_stride (i32.load offset=28 (local.get $m))))
    (local.set $live (i32.load (local.get $m)))
    (local.set $used (i32.load offset=4 (local.get $m)))
    (local.set $old (i32.load offset=8 (local.get $m)))
    (local.set $cap (select (i32.shl (local.get $live) (i32.const 1)) (i32.const 4)
      (i32.gt_u (local.get $live) (i32.const 2))))
    (local.set $slots (i32.const 8))
    (block $big
      (loop $double
        (br_if $big (i32.ge_u (i32.mul (local.get $slots) (i32.const 2)) (i32.mul (local.get $cap) (i32.const 3))))
        (local.set $slots (i32.shl (local.get $slots) (i32.const 1)))
        (br $double)))
    (local.set $entries (call $alloc (i32.mul (local.get $cap) (local.get $stride)) (i32.const 0)))
    (local.set $index (call $alloc (i32.shl (local.get $slots) (i32.const 2)) (i32.const 0)))
    (memory.fill (local.get $index) (i32.const 0) (i32.shl (local.get $slots) (i32.const 2)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $used)))
        (local.set $from (i32.add (local.get $old) (i32.mul (local.get $i) (local.get $stride))))
        (if (i32.load offset=4 (local.get $from))
          (then
            (memory.copy (i32.add (local.get $entries) (i32.mul (local.get $n) (local.get $stride)))
              (local.get $from) (local.get $stride))
            (local.set $n (i32.add (local.get $n) (i32.const 1)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (if (local.get $old) (then (call $release (local.get $old) (i32.const 0))))
    (if (i32.load offset=20 (local.get $m))
      (then (call $release (i32.load offset=20 (local.get $m)) (i32.const 0))))
    (i32.store offset=4 (local.get $m) (local.get $n))
    (i32.store offset=8 (local.get $m) (local.get $entries))
    (i32.store offset=16 (local.get $m) (local.get $cap))
    (i32.store offset=20 (local.get $m) (local.get $index))
    (i32.store offset=24 (local.get $m) (i32.sub (local.get $slots) (i32.const 1)))
    (local.set $i (i32.const 0))
    (block $placed
      (loop $place
        (br_if $placed (i32.ge_u (local.get $i) (local.get $n)))
        (call $map_place (local.get $m) (local.get $i))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $place))))

  ;; A new, empty map; `strings` when its keys are strings it owns, `shape`
  ;; its value slots and those that own a buffer.
  (func (export "map_new") (param $strings i32) (param $shape i32) (result i32)
    (local $m i32)
    (local.set $m (call $alloc (i32.const 32) (i32.const 0)))
    (memory.fill (local.get $m) (i32.const 0) (i32.const 32))
    (i32.store offset=12 (local.get $m)
      (i32.or (global.get $MAP) (select (global.get $STRKEY) (i32.const 0) (local.get $strings))))
    (i32.store offset=28 (local.get $m) (local.get $shape))
    (local.get $m))

  ;; The entry of `key`, or -1.
  (func $map_find (param $m i32) (param $key i64) (result i32)
    (local $at i32)
    (local.set $at (call $map_slot (local.get $m) (call $map_hash (local.get $m) (local.get $key)) (local.get $key)))
    (if (result i32) (local.get $at)
      (then (i32.sub (i32.load (local.get $at)) (i32.const 1)))
      (else (i32.const -1))))

  (func (export "map_find_i") (param $m i32) (param $key i64) (result i32)
    (call $map_find (local.get $m) (local.get $key)))

  (func (export "map_find_s") (param $m i32) (param $key i32) (result i32)
    (call $map_find (local.get $m) (i64.extend_i32_u (local.get $key))))

  ;; The entry of `key`, the key taken, made when it is not there: a string
  ;; key already there is released, the one in the map stays, and the
  ;; buffers its value owned are released, for the new value to be stored.
  ;; A new entry's value slots are zero.
  (func $map_put (param $m i32) (param $key i64) (result i32)
    (local $hash i32) (local $at i32) (local $e i32) (local $entry i32)
    (local.set $hash (call $map_hash (local.get $m) (local.get $key)))
    (local.set $at (call $map_slot (local.get $m) (local.get $hash) (local.get $key)))
    (if (local.get $at)
      (then
        (local.set $e (i32.sub (i32.load (local.get $at)) (i32.const 1)))
        (call $map_release_values (i32.load offset=28 (local.get $m))
          (call $map_entry (local.get $m) (local.get $e)))
        (if (call $map_strings (local.get $m))
          (then (call $buf_release (i32.wrap_i64 (local.get $key)) (i32.const 0))))
        (return (local.get $e))))
    (if (i32.ge_u (i32.load offset=4 (local.get $m)) (i32.load offset=16 (local.get $m)))
      (then (call $map_grow (local.get $m))))
    (local.set $e (i32.load offset=4 (local.get $m)))
    (local.set $entry (call $map_entry (local.get $m) (local.get $e)))
    (i32.store (local.get $entry) (local.get $hash))
    (i32.store offset=4 (local.get $entry) (i32.const 1))
    (i64.store offset=8 (local.get $entry) (local.get $key))
    (memory.fill (i32.add (local.get $entry) (i32.const 16)) (i32.const 0)
      (i32.sub (call $map_stride (i32.load offset=28 (local.get $m))) (i32.const 16)))
    (i32.store offset=4 (local.get $m) (i32.add (local.get $e) (i32.const 1)))
    (i32.store (local.get $m) (i32.add (i32.load (local.get $m)) (i32.const 1)))
    (call $map_place (local.get $m) (local.get $e))
    (local.get $e))

  (func (export "map_put_i") (param $m i32) (param $key i64) (result i32)
    (call $map_put (local.get $m) (local.get $key)))

  (func (export "map_put_s") (param $m i32) (param $key i32) (result i32)
    (call $map_put (local.get $m) (i64.extend_i32_u (local.get $key))))

  ;; The address of value slot `slot` of entry `e`.
  (func $map_at (param $m i32) (param $e i32) (param $slot i32) (result i32)
    (i32.add (call $map_entry (local.get $m) (local.get $e))
      (i32.add (i32.const 16) (i32.shl (local.get $slot) (i32.const 3)))))

  ;; Stores a scalar of a value: an integer or a boolean widened to an i64,
  ;; a float as an f64, a buffer the map now owns.
  (func (export "map_store") (param $m i32) (param $e i32) (param $slot i32) (param $v i64)
    (i64.store (call $map_at (local.get $m) (local.get $e) (local.get $slot)) (local.get $v)))

  (func (export "map_store_f") (param $m i32) (param $e i32) (param $slot i32) (param $v f64)
    (f64.store (call $map_at (local.get $m) (local.get $e) (local.get $slot)) (local.get $v)))

  (func (export "map_store_h") (param $m i32) (param $e i32) (param $slot i32) (param $v i32)
    (i64.store (call $map_at (local.get $m) (local.get $e) (local.get $slot))
      (i64.extend_i32_u (local.get $v))))

  ;; A scalar of the value of entry `e`; a buffer, lent.
  (func (export "map_value") (param $m i32) (param $e i32) (param $slot i32) (result i64)
    (i64.load (call $map_at (local.get $m) (local.get $e) (local.get $slot))))

  (func (export "map_value_f") (param $m i32) (param $e i32) (param $slot i32) (result f64)
    (f64.load (call $map_at (local.get $m) (local.get $e) (local.get $slot))))

  (func (export "map_value_h") (param $m i32) (param $e i32) (param $slot i32) (result i32)
    (i32.load (call $map_at (local.get $m) (local.get $e) (local.get $slot))))

  ;; The first live entry from `e` on, or -1 past the last: `for…of` walks
  ;; the entries in insertion order, stepping over removed ones. The loop
  ;; cannot change the map, so the entries stay where they are.
  (func (export "map_next") (param $m i32) (param $e i32) (result i32)
    (local $used i32)
    (local.set $used (i32.load offset=4 (local.get $m)))
    (block $done
      (loop $scan
        (br_if $done (i32.ge_u (local.get $e) (local.get $used)))
        (if (i32.load offset=4 (call $map_entry (local.get $m) (local.get $e)))
          (then (return (local.get $e))))
        (local.set $e (i32.add (local.get $e) (i32.const 1)))
        (br $scan)))
    (i32.const -1))

  ;; The key of entry `e`: an integer, or a string's handle, lent.
  (func (export "map_key_i") (param $m i32) (param $e i32) (result i64)
    (i64.load offset=8 (call $map_entry (local.get $m) (local.get $e))))

  (func (export "map_key_s") (param $m i32) (param $e i32) (result i32)
    (i32.wrap_i64 (i64.load offset=8 (call $map_entry (local.get $m) (local.get $e)))))

  ;; Removes `key`: 1 if it was there. Its slot becomes -1, its entry dead,
  ;; and a string key is released.
  (func $map_delete (param $m i32) (param $key i64) (result i32)
    (local $at i32) (local $entry i32)
    (local.set $at (call $map_slot (local.get $m) (call $map_hash (local.get $m) (local.get $key)) (local.get $key)))
    (if (i32.eqz (local.get $at)) (then (return (i32.const 0))))
    (local.set $entry (call $map_entry (local.get $m) (i32.sub (i32.load (local.get $at)) (i32.const 1))))
    (i32.store offset=4 (local.get $entry) (i32.const 0))
    (i32.store (local.get $at) (i32.const -1))
    (i32.store (local.get $m) (i32.sub (i32.load (local.get $m)) (i32.const 1)))
    (if (call $map_strings (local.get $m))
      (then (call $buf_release (i32.wrap_i64 (i64.load offset=8 (local.get $entry))) (i32.const 0))))
    (call $map_release_values (i32.load offset=28 (local.get $m)) (local.get $entry))
    (i32.const 1))

  (func (export "map_delete_i") (param $m i32) (param $key i64) (result i32)
    (call $map_delete (local.get $m) (local.get $key)))

  (func (export "map_delete_s") (param $m i32) (param $key i32) (result i32)
    (call $map_delete (local.get $m) (i64.extend_i32_u (local.get $key))))

  ;; Releases a map's parts: the `used` entries at `entries`, their string
  ;; keys when `strings`, the buffers their values own by `shape`, and the
  ;; index.
  (func $map_free (param $entries i32) (param $used i32) (param $strings i32) (param $index i32)
    (param $shape i32)
    (local $i i32) (local $entry i32)
    (if (i32.or (local.get $strings) (i32.shr_u (local.get $shape) (i32.const 8)))
      (then
        (block $done
          (loop $next
            (br_if $done (i32.ge_u (local.get $i) (local.get $used)))
            (local.set $entry (i32.add (local.get $entries)
              (i32.mul (local.get $i) (call $map_stride (local.get $shape)))))
            (if (i32.load offset=4 (local.get $entry))
              (then
                (if (local.get $strings)
                  (then (call $buf_release (i32.wrap_i64 (i64.load offset=8 (local.get $entry))) (i32.const 0))))
                (call $map_release_values (local.get $shape) (local.get $entry))))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next)))))
    (if (local.get $entries) (then (call $release (local.get $entries) (i32.const 0))))
    (if (local.get $index) (then (call $release (local.get $index) (i32.const 0)))))

;; A copy of map `m`: its header, entries and index, then a copy of each
  ;; string key and of each buffer a value owns, for live entries.
  (func $map_clone (param $m i32) (param $site i32) (result i32)
    (local $copy i32) (local $shape i32) (local $stride i32) (local $bytes i32)
    (local $i i32) (local $used i32) (local $entry i32) (local $owned i32) (local $at i32)
    (local.set $copy (call $alloc (i32.const 32) (local.get $site)))
    (memory.copy (local.get $copy) (local.get $m) (i32.const 32))
    (local.set $shape (i32.load offset=28 (local.get $m)))
    (local.set $stride (call $map_stride (local.get $shape)))
    (local.set $used (i32.load offset=4 (local.get $m)))
    (if (i32.eqz (i32.load offset=8 (local.get $m))) (then (return (local.get $copy))))
    (local.set $bytes (i32.mul (i32.load offset=16 (local.get $m)) (local.get $stride)))
    (i32.store offset=8 (local.get $copy) (call $alloc (local.get $bytes) (local.get $site)))
    (memory.copy (i32.load offset=8 (local.get $copy)) (i32.load offset=8 (local.get $m)) (local.get $bytes))
    (local.set $bytes (i32.shl (i32.add (i32.load offset=24 (local.get $m)) (i32.const 1)) (i32.const 2)))
    (i32.store offset=20 (local.get $copy) (call $alloc (local.get $bytes) (local.get $site)))
    (memory.copy (i32.load offset=20 (local.get $copy)) (i32.load offset=20 (local.get $m)) (local.get $bytes))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $used)))
        (local.set $entry (call $map_entry (local.get $copy) (local.get $i)))
        (if (i32.load offset=4 (local.get $entry))
          (then
            (if (call $map_strings (local.get $m))
              (then
                (i64.store offset=8 (local.get $entry)
                  (i64.extend_i32_u (call $buf_clone
                    (i32.wrap_i64 (i64.load offset=8 (local.get $entry))) (local.get $site))))))
            (local.set $owned (i32.shr_u (local.get $shape) (i32.const 8)))
            (local.set $at (i32.add (local.get $entry) (i32.const 16)))
            (block $values
              (loop $value
                (br_if $values (i32.eqz (local.get $owned)))
                (if (i32.and (local.get $owned) (i32.const 1))
                  (then
                    (i64.store (local.get $at)
                      (i64.extend_i32_u (call $buf_clone
                        (i32.wrap_i64 (i64.load (local.get $at))) (local.get $site))))))
                (local.set $owned (i32.shr_u (local.get $owned) (i32.const 1)))
                (local.set $at (i32.add (local.get $at) (i32.const 8)))
                (br $value)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.get $copy))

  ;; Empties the map: its string keys released, its blocks given back.
  (func $map_clear (export "map_clear") (param $m i32)
    (call $map_free (i32.load offset=8 (local.get $m)) (i32.load offset=4 (local.get $m))
      (call $map_strings (local.get $m)) (i32.load offset=20 (local.get $m))
      (i32.load offset=28 (local.get $m)))
    (i32.store (local.get $m) (i32.const 0))
    (i32.store offset=4 (local.get $m) (i32.const 0))
    (i32.store offset=8 (local.get $m) (i32.const 0))
    (i32.store offset=16 (local.get $m) (i32.const 0))
    (i32.store offset=20 (local.get $m) (i32.const 0))
    (i32.store offset=24 (local.get $m) (i32.const 0)))
