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
  ;; The elements are a block of their own, `capacity * size` bytes. Every
  ;; `size` argument below may carry DEEP; only its low byte is a size.

  ;; A new, empty buffer with room for `cap` elements of `size` bytes.
  (func $buf_new (export "buf_new") (param $cap i32) (param $size i32) (param $site i32) (result i32)
    (local $h i32)
    (local.set $h (call $alloc (i32.const 16) (local.get $site)))
    (i32.store (local.get $h) (i32.const 0))
    (i32.store offset=4 (local.get $h) (local.get $cap))
    (i32.store offset=8 (local.get $h)
      (if (result i32) (i32.eqz (local.get $cap))
        (then (i32.const 0))
        (else (call $alloc (call $bytes (local.get $cap) (local.get $size) (local.get $site)) (local.get $site)))))
    (i32.store offset=12 (local.get $h) (local.get $size))
    (local.get $h))

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
        (call $release (i32.load offset=8 (local.get $h)) (local.get $site))))
    (i32.store offset=4 (local.get $h) (local.get $new_cap))
    (i32.store offset=8 (local.get $h) (local.get $data)))

  ;; Releases a buffer: its header first, which catches a buffer released twice
  ;; before its poisoned contents are read, then the buffers it owns, if its
  ;; elements are buffers, then its elements.
  (func $buf_release (export "buf_release") (param $h i32) (param $site i32)
    (local $data i32) (local $len i32) (local $deep i32) (local $i i32)
    (local.set $data (i32.load offset=8 (local.get $h)))
    (local.set $len (i32.load (local.get $h)))
    (local.set $deep (i32.and (i32.load offset=12 (local.get $h)) (global.get $DEEP)))
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
    (local.set $len (i32.load (local.get $h)))
    (local.set $size (i32.load offset=12 (local.get $h)))
    (local.set $copy (call $buf_new (local.get $len) (local.get $size) (local.get $site)))
    (if (local.get $len)
      (then
        (memory.copy (i32.load offset=8 (local.get $copy)) (i32.load offset=8 (local.get $h))
          (call $bytes (local.get $len) (local.get $size) (local.get $site)))))
    (i32.store (local.get $copy) (local.get $len))
    (if (i32.and (local.get $size) (global.get $DEEP))
      (then
        (block $done
          (loop $next
            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
            (local.set $at (i32.add (i32.load offset=8 (local.get $copy)) (i32.shl (local.get $i) (i32.const 2))))
            (i32.store (local.get $at) (call $buf_clone (i32.load (local.get $at)) (local.get $site)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $next)))))
    (local.get $copy))
