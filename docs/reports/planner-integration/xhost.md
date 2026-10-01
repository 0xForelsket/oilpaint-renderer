Cross-host check, engine 2.0.0-dev.7, 5 hosts, no golden for this engine version

| case | expect | chromium-145.0.7632.6 | firefox-146.0.1 | native-windows-x86_64 | node-v24.21.0-win32-x64 | webkit-26.0 | verdict |
|---|---|---|---|---|---|---|---|
| control.libm.exp | may_differ | 3d0196939b97 | 3d0196939b97 | 0e3d8038bac1 | 3d0196939b97 | 3d0196939b97 | differs (negative control, 2 digests) |
| control.libm.powf_srgb | may_differ | 1ae7600e8eea | 1ae7600e8eea | c3592752d083 | 1ae7600e8eea | 1ae7600e8eea | differs (negative control, 2 digests) |
| kernel.testsheet@mixbox@400.cover | identical | 1bb9625c4922 | 1bb9625c4922 | 1bb9625c4922 | 1bb9625c4922 | 1bb9625c4922 | identical |
| kernel.testsheet@mixbox@400.h | identical | bc7fbde163b8 | bc7fbde163b8 | bc7fbde163b8 | bc7fbde163b8 | bc7fbde163b8 | identical |
| kernel.testsheet@mixbox@400.lat | identical | af89eec43763 | af89eec43763 | af89eec43763 | af89eec43763 | af89eec43763 | identical |
| kernel.testsheet@mixbox@400.rgb | identical | 7d37f7f5face | 7d37f7f5face | 7d37f7f5face | 7d37f7f5face | 7d37f7f5face | identical |
| kernel.testsheet@mixbox@400.wet | identical | 91b3c162a20b | 91b3c162a20b | 91b3c162a20b | 91b3c162a20b | 91b3c162a20b | identical |
| kernel.testsheet@ochrell@400.cover | identical | 1bb9625c4922 | 1bb9625c4922 | 1bb9625c4922 | 1bb9625c4922 | 1bb9625c4922 | identical |
| kernel.testsheet@ochrell@400.h | identical | bc7fbde163b8 | bc7fbde163b8 | bc7fbde163b8 | bc7fbde163b8 | bc7fbde163b8 | identical |
| kernel.testsheet@ochrell@400.lat | identical | fc61408f95c6 | fc61408f95c6 | fc61408f95c6 | fc61408f95c6 | fc61408f95c6 | identical |
| kernel.testsheet@ochrell@400.rgb | identical | ba9ed18dc58b | ba9ed18dc58b | ba9ed18dc58b | ba9ed18dc58b | ba9ed18dc58b | identical |
| kernel.testsheet@ochrell@400.wet | identical | 91b3c162a20b | 91b3c162a20b | 91b3c162a20b | 91b3c162a20b | 91b3c162a20b | identical |
| kernel.testsheet@rgb@400.cover | identical | 1bb9625c4922 | 1bb9625c4922 | 1bb9625c4922 | 1bb9625c4922 | 1bb9625c4922 | identical |
| kernel.testsheet@rgb@400.h | identical | bc7fbde163b8 | bc7fbde163b8 | bc7fbde163b8 | bc7fbde163b8 | bc7fbde163b8 | identical |
| kernel.testsheet@rgb@400.lat | identical | 94d1d2b97af0 | 94d1d2b97af0 | 94d1d2b97af0 | 94d1d2b97af0 | 94d1d2b97af0 | identical |
| kernel.testsheet@rgb@400.rgb | identical | f8091268e030 | f8091268e030 | f8091268e030 | f8091268e030 | f8091268e030 | identical |
| kernel.testsheet@rgb@400.wet | identical | 91b3c162a20b | 91b3c162a20b | 91b3c162a20b | 91b3c162a20b | 91b3c162a20b | identical |
| light.testsheet@mixbox@400.lit | identical | b0b4621327f9 | b0b4621327f9 | b0b4621327f9 | b0b4621327f9 | b0b4621327f9 | identical |
| light.testsheet@ochrell@400.lit | identical | 1d3071e669d1 | 1d3071e669d1 | 1d3071e669d1 | 1d3071e669d1 | 1d3071e669d1 | identical |
| light.testsheet@rgb@400.lit | identical | 805bd519c2e5 | 805bd519c2e5 | 805bd519c2e5 | 805bd519c2e5 | 805bd519c2e5 | identical |
| math.exp.f64 | identical | 96c165b5039a | 96c165b5039a | 96c165b5039a | 96c165b5039a | 96c165b5039a | identical |
| math.expf.f32 | identical | a422eb3d4f5b | a422eb3d4f5b | a422eb3d4f5b | a422eb3d4f5b | a422eb3d4f5b | identical |
| math.sincos.f64 | identical | 95bef6dd8e06 | 95bef6dd8e06 | 95bef6dd8e06 | 95bef6dd8e06 | 95bef6dd8e06 | identical |
| plan.still_life@300.ochrell | identical | e782efe98f94 | e782efe98f94 | e782efe98f94 | e782efe98f94 | e782efe98f94 | identical |
| plan.storm_v3@300.ochrell | identical | ef2aeb410069 | ef2aeb410069 | ef2aeb410069 | ef2aeb410069 | ef2aeb410069 | identical |
| scene.storm_v3@600.flow | identical | 786793e9e3c2 | 786793e9e3c2 | 786793e9e3c2 | 786793e9e3c2 | 786793e9e3c2 | identical |
| scene.storm_v3@600.flowPoints | identical | f2068ff26457 | f2068ff26457 | f2068ff26457 | f2068ff26457 | f2068ff26457 | identical |
| scene.storm_v3@600.light | identical | e62f0cf3c6c3 | e62f0cf3c6c3 | e62f0cf3c6c3 | e62f0cf3c6c3 | e62f0cf3c6c3 | identical |
| scene.storm_v3@600.masks | identical | 404bd3d45230 | 404bd3d45230 | 404bd3d45230 | 404bd3d45230 | 404bd3d45230 | identical |
| scene.storm_v3@600.regionId | identical | f452cd527b8c | f452cd527b8c | f452cd527b8c | f452cd527b8c | f452cd527b8c | identical |
| scene.storm_v3@600.target | identical | a5acb829e61a | a5acb829e61a | a5acb829e61a | a5acb829e61a | a5acb829e61a | identical |
| scene.validate.codes | identical | 31482fc2bccf | 31482fc2bccf | 31482fc2bccf | 31482fc2bccf | 31482fc2bccf | identical |
| strokes.testsheet | identical | ab60e67ecc18 | ab60e67ecc18 | ab60e67ecc18 | ab60e67ecc18 | ab60e67ecc18 | identical |

PASS
