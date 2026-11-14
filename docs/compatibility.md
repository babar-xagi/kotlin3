# 🛡️ Kotlin 2.x Compatibility Specification

A primary design requirement of **Kotlin3** is **100% binary and semantic compatibility** with existing Kotlin code.

---

## 1. Frontend Semantic Compatibility (K2 / FIR)

Kotlin3 aligns with the **Kotlin 2.0+ Frontend Intermediate Representation (FIR)** pipeline:

### Phased Resolution Alignment
1. **Raw FIR**: Syntactic transformation preserving AST spans.
2. **Imports**: Standard wildcard and named import resolution rules.
3. **SuperTypes**: Supertypes must be resolved before class member signatures.
4. **Types**: Signatures resolved before expression bodies.
5. **Status**: Modality and visibility resolution (`public`, `internal`, `protected`, `private`).
6. **Contracts**: Function contracts (`callsInPlace`, `returns(true) implies ...`).
7. **Implicit Types**: Local body and return type inference.
8. **Body Resolve**: Full expression typing via **Cone Types** and **PCLA** (Partially Constrained Lambda Arguments).
9. **Diagnostics & Checkers**: Emitting standard diagnostics matching official compiler error codes.

---

## 2. JVM Binary Compatibility

### Java Classfile Compliance
- Outputs standard `.class` files compatible with **Java 8 through Java 25**.
- Correct stack map frames (`StackMapTable`) computed directly during compilation.
- Proper method parameter descriptors and generic signatures.

### `@kotlin.Metadata` Annotation
Existing Kotlin tooling and reflection rely on the `@kotlin.Metadata` annotation attached to compiled `.class` files. Kotlin3 emits binary-identical Protobuf-encoded `@kotlin.Metadata` containing:
- `k`: Kind of declaration (1 = class, 2 = file, 3 = synthetic class, etc.).
- `mv`: Metadata version (e.g., `[2, 0, 0]`).
- `d1`: Byte array containing Protobuf-serialized declaration table.
- `d2`: String array table for deduplicated names.

---

## 3. Dependency Interoperability

Kotlin3 seamlessly consumes:
- Standard Maven Central `.jar` libraries (deserializing Kotlin `@Metadata` and Java bytecode signatures).
- Official `.klib` packages produced by Kotlin Multiplatform tooling.
