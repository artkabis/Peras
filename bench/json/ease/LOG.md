# Writing the programs of decision 0038: the errors met

Each program written once, as a user would, then built; every error the compiler reported before it built is listed here, with what fixed it. The counts of lines and notions are made by `scripts/bench-json.mjs`.

| Option | Program | Errors met                                                                                                                              |
| ------ | ------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| j0     | P1      | none                                                                                                                                    |
| j0     | P2      | none                                                                                                                                    |
| j0     | P3      | none                                                                                                                                    |
| j1     | P1      | none                                                                                                                                    |
| j1     | P2      | none; `fromJson` is called on a structure, so the array is read through `Records`, a structure that holds it                            |
| j1     | P3      | none                                                                                                                                    |
| j2     | P1      | E0203, `i` never read: the index meant for the path `servers[i]` not yet passed on; fixed by giving each reader the path before the key |
| j2     | P2      | none                                                                                                                                    |
| j2     | P3      | none                                                                                                                                    |
