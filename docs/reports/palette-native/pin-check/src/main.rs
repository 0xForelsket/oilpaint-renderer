fn main() {
    let mut seed=9371_u64;
    for i in 0..10000 {
        let mut color=|| std::array::from_fn(|_|{
            seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;
            (seed>>11) as f64/(1_u64<<53) as f64
        });
        let a=color();let b=color();
        let old=before::FastPigmentMixer;
        let new=after::FastPigmentMixer;
        let oa=old.encode_linear(a).unwrap();let ob=old.encode_linear(b).unwrap();
        let na=new.encode_linear(a).unwrap();let nb=new.encode_linear(b).unwrap();
        for (o,n) in [(oa,na),(ob,nb),(oa.interpolate(ob,(i%101) as f32/100.0),na.interpolate(nb,(i%101) as f32/100.0))] {
            assert_eq!(o.absorption().map(f32::to_bits),n.absorption().map(f32::to_bits));
            assert_eq!(o.scattering().map(f32::to_bits),n.scattering().map(f32::to_bits));
            assert_eq!(o.residual().map(f32::to_bits),n.residual().map(f32::to_bits));
            assert_eq!(old.decode_linear(o).map(f64::to_bits),new.decode_linear(n).map(f64::to_bits));
        }
    }
    println!("PASS: 10000 color pairs; 30000 encoded/mixed states and linear decodes bit-identical across old/new pins");
}
