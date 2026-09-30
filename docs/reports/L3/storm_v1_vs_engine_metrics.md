# v1 planner vs engine planner: storm_v3.py at 600 px (ochrell, seed 1907)

For information: the planner port is a cutover (no parity gate). Both plans painted by the same `oil paint`.

Strokes: v1 13486, engine 13439. Planning (unpinned wall clock): v1 64.2 s, engine 10.7 s.

| layer | v1 strokes | engine strokes |
|---|---|---|
| Toned ground | 24 | 31 |
| Ebauche dark masses | 1902 | 1866 |
| Ebauche light masses | 500 | 526 |
| Sky long strokes | 1174 | 1165 |
| Storm swirl | 1604 | 1541 |
| Sea underpainting | 1361 | 1668 |
| Rock | 1012 | 1046 |
| Tower and lamp | 3526 | 3336 |
| Broken colour | 1341 | 1230 |
| Glow and halo scumble | 15 | 23 |
| Beam and band edges | 203 | 181 |
| Wave crests and foam | 181 | 199 |
| Lost edges | 12 | 8 |
| Glaze and rain | 377 | 398 |
| Lantern and impasto touches | 254 | 221 |

| region | metric | v1 | engine |
|---|---|---|---|
| sky | hairline | 0.038 | 0.046 |
| sky | bristle_L | 0.988 | 1.156 |
| sky | edge_step_p99 | 4.285 | 2.877 |
| sky | h_p95 | 13.705 | 12.960 |
| sky | C_mean | 11.347 | 10.912 |
| sky | L_p50 | 70.005 | 70.757 |
| storm | hairline | 0.053 | 0.044 |
| storm | bristle_L | 1.293 | 1.320 |
| storm | edge_step_p99 | 2.328 | 1.973 |
| storm | h_p95 | 15.547 | 13.414 |
| storm | C_mean | 29.697 | 29.649 |
| storm | L_p50 | 36.084 | 36.467 |
| glow | hairline | 0.060 | 0.059 |
| glow | bristle_L | 0.904 | 0.905 |
| glow | edge_step_p99 | 3.839 | 2.290 |
| glow | h_p95 | 8.210 | 7.810 |
| glow | C_mean | 14.580 | 14.624 |
| glow | L_p50 | 81.143 | 81.640 |
| sea_far | hairline | 0.086 | 0.099 |
| sea_far | bristle_L | 1.325 | 1.322 |
| sea_far | edge_step_p99 | 6.650 | 5.127 |
| sea_far | h_p95 | 15.790 | 16.001 |
| sea_far | C_mean | 8.983 | 8.572 |
| sea_far | L_p50 | 80.365 | 79.671 |
| sea_near | hairline | 0.133 | 0.122 |
| sea_near | bristle_L | 2.228 | 2.050 |
| sea_near | edge_step_p99 | 4.746 | 3.199 |
| sea_near | h_p95 | 32.426 | 43.633 |
| sea_near | C_mean | 23.341 | 24.014 |
| sea_near | L_p50 | 47.714 | 48.539 |
| rock | hairline | 0.134 | 0.115 |
| rock | bristle_L | 1.487 | 1.648 |
| rock | edge_step_p99 | 4.030 | 3.881 |
| rock | h_p95 | 10.595 | 10.405 |
| rock | C_mean | 23.351 | 22.899 |
| rock | L_p50 | 42.568 | 43.351 |
| crest | hairline | 0.105 | 0.108 |
| crest | bristle_L | 2.239 | 2.243 |
| crest | edge_step_p99 | 4.595 | 5.570 |
| crest | h_p95 | 19.171 | 23.133 |
| crest | C_mean | 25.807 | 26.107 |
| crest | L_p50 | 50.958 | 49.755 |

| region | v1 width med | engine width med | v1 length med | engine length med | v1 n | engine n |
|---|---|---|---|---|---|---|
| ground | 0.0897 | 0.0913 | 0.425 | 0.390 | 24 | 31 |
| rain | 0.0054 | 0.0058 | 0.123 | 0.101 | 192 | 211 |
| sky | 0.0399 | 0.0412 | 0.206 | 0.182 | 2308 | 2343 |
| storm | 0.0283 | 0.0295 | 0.119 | 0.109 | 1968 | 1857 |
| glow | 0.0265 | 0.0283 | 0.037 | 0.044 | 50 | 44 |
| sea_far | 0.0193 | 0.0220 | 0.093 | 0.083 | 1310 | 1227 |
| sea_near | 0.0241 | 0.0281 | 0.115 | 0.106 | 2179 | 2460 |
| rock | 0.0191 | 0.0193 | 0.064 | 0.064 | 606 | 710 |
| foam | 0.0113 | 0.0122 | 0.048 | 0.040 | 609 | 514 |
| spray | 0.0072 | 0.0083 | 0.011 | 0.022 | 5 | 9 |
| tower | 0.0117 | 0.0110 | 0.055 | 0.044 | 1100 | 1057 |
| band1 | 0.0083 | 0.0077 | 0.031 | 0.025 | 497 | 466 |
| band2 | 0.0082 | 0.0078 | 0.033 | 0.027 | 622 | 556 |
| beam | 0.0204 | 0.0225 | 0.044 | 0.025 | 37 | 36 |
| halo | 0.0082 | 0.0076 | 0.019 | 0.017 | 1198 | 1176 |
| lantern | 0.0065 | 0.0062 | 0.023 | 0.019 | 281 | 238 |
| roof | 0.0059 | 0.0050 | 0.018 | 0.015 | 174 | 177 |
| foot | 0.0069 | 0.0061 | 0.025 | 0.020 | 49 | 41 |
| gallery | 0.0053 | 0.0049 | 0.028 | 0.023 | 174 | 155 |
| crest | 0.0096 | 0.0098 | 0.082 | 0.065 | 103 | 131 |
