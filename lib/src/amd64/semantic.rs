use value::{Lvalue,Rvalue,Endianess};
use codegen::CodeGen;
use disassembler::State;
use amd64::*;
use guard::Guard;

fn do_push(v: &Rvalue, mode: Mode, cg: &mut CodeGen<Amd64>) {
    if let &Rvalue::Variable{ width: w, ..} = v {
	    cg.assign(&Lvalue::Memory{
            offset: Box::new(rip.to_rv()),
            bytes: w / 8,
            endianess: Endianess::Little,
            name: "ram".to_string()
        },v);

        match mode {
		    Mode::Real => {
                cg.add_i(&*sp,&sp.to_rv(),&Rvalue::Constant(w as u64));
                cg.mod_i(&*sp,&sp.to_rv(),&Rvalue::Constant(0x10000));
            }
		    Mode::Protected => {
                cg.add_i(&*esp,&esp.to_rv(),&Rvalue::Constant(w as u64));
                cg.mod_i(&*esp,&esp.to_rv(),&Rvalue::Constant(0x100000000));
            }

		    Mode::Long => {
                cg.add_i(&*rsp,&rsp.to_rv(),&Rvalue::Constant(w as u64));
            }
	    }
    } else {
        unreachable!()
    }
}

fn bitwidth(a: &Rvalue) -> usize {
    match a {
        &Rvalue::Variable{ width: w, .. } => w as usize,
        &Rvalue::Memory{ bytes: b, .. } => (b as usize) * 8,
        _ => unreachable!()
    }
}

fn sign_ext(v: &Rvalue, from: usize, to: usize, cg: &mut CodeGen<Amd64>) -> Rvalue {
    assert!(from < to  && from > 0);

    let sign = new_temp(to);
    let rest = new_temp(to);
    let mask = Rvalue::Constant(1 << (from - 1));

    cg.div_i(&sign,v,&mask);
    cg.mod_i(&rest,v,&mask);

    cg.mod_i(&sign,&sign.to_rv(),&Rvalue::Constant(1 << (to - 1)));
    cg.add_i(&rest,&sign.to_rv(),&rest.to_rv());

    rest.to_rv()
}

fn set_arithm_flags(res: &Lvalue, res_half: &Rvalue, a: &Rvalue, b: &Rvalue, cg: &mut CodeGen<Amd64>) {
	let aw = bitwidth(a);

    cg.div_i(&*CF,&res.to_rv(),&Rvalue::Constant(1 << aw));
	cg.div_i(&*AF,res_half,&Rvalue::Constant(0x100));
    cg.div_i(&*SF,&res.to_rv(),&Rvalue::Constant(1 << (aw - 1)));
	cg.equal_i(&*ZF,a, &Rvalue::Constant(0));
	cg.xor_i(&*OF,&CF.to_rv(),&SF.to_rv());

    let tmp = new_temp(aw);

    cg.mod_i(&*PF,&res.to_rv(),&Rvalue::Constant(2));

    cg.mod_i(&tmp,&res.to_rv(),&Rvalue::Constant(4));
    cg.div_i(&tmp,&res.to_rv(),&Rvalue::Constant(2));
    cg.xor_i(&*PF,&*PF,&tmp.to_rv());

    cg.mod_i(&tmp,&res.to_rv(),&Rvalue::Constant(8));
    cg.div_i(&tmp,&res.to_rv(),&Rvalue::Constant(4));
    cg.xor_i(&*PF,&*PF,&tmp.to_rv());

    cg.mod_i(&tmp,&res.to_rv(),&Rvalue::Constant(16));
    cg.div_i(&tmp,&res.to_rv(),&Rvalue::Constant(8));
    cg.xor_i(&*PF,&*PF,&tmp.to_rv());

    cg.mod_i(&tmp,&res.to_rv(),&Rvalue::Constant(32));
    cg.div_i(&tmp,&res.to_rv(),&Rvalue::Constant(16));
    cg.xor_i(&*PF,&*PF,&tmp.to_rv());

    cg.mod_i(&tmp,&res.to_rv(),&Rvalue::Constant(64));
    cg.div_i(&tmp,&res.to_rv(),&Rvalue::Constant(32));
    cg.xor_i(&*PF,&*PF,&tmp.to_rv());

    cg.mod_i(&tmp,&res.to_rv(),&Rvalue::Constant(128));
    cg.div_i(&tmp,&res.to_rv(),&Rvalue::Constant(64));
    cg.xor_i(&*PF,&*PF,&tmp);

    cg.mod_i(&tmp,&res.to_rv(),&Rvalue::Constant(256));
    cg.div_i(&tmp,&res.to_rv(),&Rvalue::Constant(128));
    cg.xor_i(&*PF,&*PF,&tmp.to_rv());
}

/*fn flagcomp(_: &mut CodeGen<Amd64>, variable const& flag)
{
}*/

pub fn flagwr(flag: &Lvalue, val: bool) -> Box<Fn(&mut CodeGen<Amd64>)> {
    let f = flag.clone();
    Box::new(move |cg: &mut CodeGen<Amd64>| {
        cg.assign(&f,&Rvalue::Constant(if val { 1 } else { 0 }));
    })
}

pub fn flagcomp(flag: &Lvalue) -> Box<Fn(&mut CodeGen<Amd64>)> {
    let f = flag.clone();
    Box::new(move |cg: &mut CodeGen<Amd64>| {
        cg.not_b(&f,&f);
    })
}

pub fn adc(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	size_t const a_w = bitwidth(a), b_w = (is_Rvalue::Constant(b) ? a_w : bitwidth(b));
	rvalue const res = a + (a_w == b_w ? b : sign_ext(b,b_w,a_w,m)) + CF;
	rvalue const res_half = (a % Rvalue::Constant(0x100)) + (b % constant(0x100)) + CF;

	m.assign(to_lvalue(a),res % Rvalue::Constant(1 << a_w));
	set_arithm_flags(res,res_half,a,b,m);*/
}

pub fn aaa(cg: &mut CodeGen<Amd64>) {
    let y = new_temp(16);
    let x1 = new_temp(1);
    let x2 = new_temp(1);

    cg.and_b(&y,&*al,&Rvalue::Constant(0x0f));

    // x1 = !(y <= 9) || AF
    cg.equal_i(&x1,&y.to_rv(),&Rvalue::Constant(9));
    cg.less_i(&x2,&y.to_rv(),&Rvalue::Constant(9));
    cg.or_b(&x1,&x1.to_rv(),&x2.to_rv());
    cg.not_b(&x1,&x1.to_rv());
    cg.or_b(&x1,&x1.to_rv(),&AF.to_rv());

    cg.assign(&*AF,&x1.to_rv());
    cg.assign(&*CF,&x1.to_rv());

    // ax = (ax + x1 * 0x106) % 0x100
    cg.lift_b(&y,&x1.to_rv());
    cg.mul_i(&y,&y.to_rv(),&Rvalue::Constant(0x106));
    cg.add_i(&ax,&ax.to_rv(),&y.to_rv());
    cg.mod_i(&ax,&ax.to_rv(),&Rvalue::Constant(0x100));
}

pub fn aam(cg: &mut CodeGen<Amd64>, a: Rvalue) {
    let temp_al = new_temp(16);

    cg.assign(&temp_al,&al.to_rv());
    cg.div_i(&*ah,&temp_al,&a);
    cg.mod_i(&*al,&temp_al,&a);
}

pub fn aad(cg: &mut CodeGen<Amd64>, a: Rvalue) {
    let x = new_temp(16);

    cg.mul_i(&x,&ah.to_rv(),&a);
    cg.add_i(&*al,&x,&al.to_rv());
    cg.assign(&*ah,&Rvalue::Constant(0));
}

pub fn aas(cg: &mut CodeGen<Amd64>) {
    let y1 = new_temp(16);
    let x1 = new_temp(1);
    let x2 = new_temp(1);

    cg.and_b(&y1,&*al,&Rvalue::Constant(0x0f));

    // x1 = !(y <= 9) || AF
    cg.equal_i(&x1,&y1.to_rv(),&Rvalue::Constant(9));
    cg.less_i(&x2,&y1.to_rv(),&Rvalue::Constant(9));
    cg.or_b(&x1,&x1.to_rv(),&x2.to_rv());
    cg.not_b(&x1,&x1.to_rv());
    cg.or_b(&x1,&x1.to_rv(),&AF.to_rv());

    cg.assign(&*AF,&x1.to_rv());
    cg.assign(&*CF,&x1.to_rv());

    let y2 = new_temp(16);

    // ax = (ax - x1 * 6) % 0x100
    cg.lift_b(&y2,&x1.to_rv());
    cg.mul_i(&y2,&y2.to_rv(),&Rvalue::Constant(6));
    cg.sub_i(&ax,&ax.to_rv(),&y2.to_rv());
    cg.mod_i(&ax,&ax.to_rv(),&Rvalue::Constant(0x100));

    let z = new_temp(16);

    // ah = (ah - x1) % 0x10
    cg.lift_b(&z,&x1.to_rv());
    cg.sub_i(&ah,&ah.to_rv(),&z.to_rv());
    cg.mod_i(&ah,&ah.to_rv(),&Rvalue::Constant(0x10));

    cg.assign(&*al,&y1.to_rv());
}

pub fn add(cg: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	size_t const a_w = bitwidth(a), b_w = (is_Rvalue::Constant(b) ? a_w : bitwidth(b));
	rvalue const res = a + (a_w == b_w ? b : sign_ext(b,b_w,a_w,m));
	rvalue const res_half = (a % Rvalue::Constant(0x100)) + (b % constant(0x100));

	m.assign(to_lvalue(a),res % Rvalue::Constant(1 << a_w));
	set_arithm_flags(res,res_half,a,b,m);*/
}

pub fn adcx(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	size_t const a_w = bitwidth(a);
	rvalue const res = a + b + CF;

	m.assign(to_lvalue(CF), res / Rvalue::Constant(1 << a_w));
	m.assign(to_lvalue(a),res % Rvalue::Constant(1 << a_w));*/
}

pub fn and(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	unsigned int a_w = bitwidth(a), b_w = (is_Rvalue::Constant(b) ? a_w : bitwidth(b));
	rvalue const res = a & (a_w == b_w ? b : sign_ext(b,b_w,a_w,m));
	rvalue const res_half = (a % Rvalue::Constant(0x100)) & (b % constant(0x100));

	m.assign(to_lvalue(a),res);
	set_arithm_flags(res,res_half,a,b,m);*/
}

pub fn arpl(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}

pub fn bound(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}

pub fn bsf(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	using dsl::operator*;

	size_t const a_w = bitwidth(a);
	size_t bit = 0;
	boost::optional<rvalue> prev;

	m.assign(to_lvalue(ZF), equal(Rvalue::Constant(0), b));

	while(bit < a_w)
	{
		rvalue val = (b % (1 << (bit + 1)) / (1 << bit));

		m.assign(to_lvalue(a),Rvalue::Constant(bit + 1) * val);
		if(prev)
			prev = *prev | val;
		else
			prev = val;

		++bit;
	}*/
}

pub fn bsr(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	using dsl::operator*;

	size_t const a_w = bitwidth(a);
	size_t bit = a_w - 1;
	boost::optional<rvalue> prev;

	m.assign(to_lvalue(ZF), equal(Rvalue::Constant(0), b));

	do
	{
		rvalue val = (b % (1 << (bit + 1)) / (1 << bit));

		m.assign(to_lvalue(a),Rvalue::Constant(bit + 1) * val);
		if(prev)
			prev = *prev | val;
		else
			prev = val;
	}
	while(bit--);*/
}

pub fn bswap(_: &mut CodeGen<Amd64>, a: Rvalue) {
    //unimplemented!()
    /*
	using dsl::operator*;

	size_t const a_w = bitwidth(a);
	size_t byte = 0;

	rvalue tmp = undefined();

	while(byte < a_w / 8)
	{
		unsigned int lsb = byte * 8;
		unsigned int div = (1 << lsb), mul = (1 << (a_w - byte * 8));

		tmp = tmp + (((a / div) % Rvalue::Constant(0x100)) * mul);
		++byte;
	}

	m.assign(to_lvalue(a),tmp);*/
}

pub fn bt(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	using dsl::operator<<;
	rvalue mod = (Rvalue::Constant(1) << (b % constant(bitwidth(a))));

	m.assign(to_lvalue(CF), (a / mod) % 2);
	m.assign(to_lvalue(PF), undefined());
	m.assign(to_lvalue(OF), undefined());
	m.assign(to_lvalue(SF), undefined());
	m.assign(to_lvalue(AF), undefined());*/
}

pub fn btc(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	using dsl::operator<<;
	rvalue mod = (Rvalue::Constant(1) << (b % constant(bitwidth(a))));

	m.assign(to_lvalue(CF), (a / mod) % 2);
	m.assign(to_lvalue(PF), undefined());
	m.assign(to_lvalue(OF), undefined());
	m.assign(to_lvalue(SF), undefined());
	m.assign(to_lvalue(AF), undefined());
	m.assign(to_lvalue(a),a ^ mod);*/
}

pub fn btr(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	using dsl::operator<<;
	size_t const a_w = bitwidth(a);
	rvalue mod =  ((Rvalue::Constant(1) << (b % constant(bitwidth(a)))));

	m.assign(to_lvalue(CF), (a / mod) % 2);
	m.assign(to_lvalue(PF), undefined());
	m.assign(to_lvalue(OF), undefined());
	m.assign(to_lvalue(SF), undefined());
	m.assign(to_lvalue(AF), undefined());
	m.assign(to_lvalue(a),(a & (Rvalue::Constant(0xffffffffffffffff) ^ mod)) % constant(1 << a_w));*/
}

pub fn bts(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    //unimplemented!()
    /*
	using dsl::operator<<;
	rvalue mod = (Rvalue::Constant(1) << (b % constant(bitwidth(a))));

	m.assign(to_lvalue(CF), (a / mod) % 2);
	m.assign(to_lvalue(PF), undefined());
	m.assign(to_lvalue(OF), undefined());
	m.assign(to_lvalue(SF), undefined());
	m.assign(to_lvalue(AF), undefined());
	m.assign(to_lvalue(a),a & mod);*/
}

pub fn near_call(cg: &mut CodeGen<Amd64>, a: Rvalue) {
    near_xcall(cg,a,false)
}

pub fn near_rcall(cg: &mut CodeGen<Amd64>, a: Rvalue) {
    near_xcall(cg,a,true)
}

pub fn near_xcall(cg: &mut CodeGen<Amd64>, a: Rvalue, rel: bool) {
    match cg.configuration.operand_size {
        OperandSize::Sixteen => {
			let new_ip = if rel {
				let x = Lvalue::from_rvalue(&sign_ext(&a,32,64,cg)).unwrap();
                cg.add_i(&x,&x.to_rv(),&rip.to_rv());
                x
            } else {
				Lvalue::from_rvalue(&sign_ext(&a,32,64,cg)).unwrap()
            };

			do_push(&rip.to_rv(),Mode::Long,cg);
			cg.assign(&*rip, &new_ip);
			cg.call_i(&Lvalue::Undefined,&new_ip);
		},
        OperandSize::ThirtyTwo => {
			let new_ip = if rel {
                let x = new_temp(32);
                cg.add_i(&x,&a,&eip.to_rv());
                cg.mod_i(&x,&x,&Rvalue::Constant(0x100000000));
                x
            } else {
                Lvalue::from_rvalue(&a).unwrap()
            };

			do_push(&eip.to_rv(),Mode::Protected,cg);
			cg.assign(&*eip, &new_ip);
			cg.call_i(&Lvalue::Undefined,&new_ip);
		},
        OperandSize::SixtyFour => {
			let new_ip = if rel {
                let x = new_temp(16);
                cg.add_i(&x,&a,&eip.to_rv());
                cg.mod_i(&x,&x.to_rv(),&Rvalue::Constant(0x10000));
                x
            } else {
                let x = new_temp(16);
                cg.mod_i(&x,&a,&Rvalue::Constant(0x10000));
                x
            };

			do_push(&rip.to_rv(),Mode::Real,cg);
			cg.assign(&*rip, &new_ip);
			cg.call_i(&Lvalue::Undefined,&new_ip);
		}
	    OperandSize::HundredTwentyEight => unreachable!(),
		OperandSize::Eight => unreachable!(),
	}
}

pub fn far_call(cg: &mut CodeGen<Amd64>, a: Rvalue) {
    far_xcall(cg,a,false)
}

pub fn far_rcall(cg: &mut CodeGen<Amd64>, a: Rvalue) {
    far_xcall(cg,a,true)
}

pub fn far_xcall(cg: &mut CodeGen<Amd64>, a: Rvalue, rel: bool) {
    match cg.configuration.operand_size {
		OperandSize::Sixteen => {
			do_push(&cs.to_rv(),Mode::Real,cg);
			do_push(&ip.to_rv(),Mode::Real,cg);
		},
		OperandSize::ThirtyTwo => {
			do_push(&cs.to_rv(),Mode::Protected,cg);
			do_push(&eip.to_rv(),Mode::Protected,cg);
		},
		OperandSize::SixtyFour => {
			do_push(&cs.to_rv(),Mode::Long,cg);
			do_push(&rip.to_rv(),Mode::Long,cg);
		},
		OperandSize::HundredTwentyEight => unreachable!(),
		OperandSize::Eight => unreachable!(),
	}
}

pub fn cmov(cg: &mut CodeGen<Amd64>, _a: Rvalue, b: Rvalue, c: Condition) {
    let a = Lvalue::from_rvalue(&_a).unwrap();
    let fun = |f: &Lvalue,cg: &mut CodeGen<Amd64>| {
        let tmp = new_temp(bitwidth(&a.to_rv()));
        let l = new_temp(bitwidth(&a.to_rv()));
        let nl = new_temp(bitwidth(&a.to_rv()));
        let n = new_temp(1);

        cg.lift_b(&l,&f.to_rv());
        cg.not_b(&n,&f.to_rv());
        cg.lift_b(&nl,&n);
        cg.mul_i(&l,&l,&b);
        cg.mul_i(&nl,&nl,&a.to_rv());
        cg.add_i(&a,&l,&nl);
    };

    match c {
		Condition::Overflow => fun(&*OF,cg),
		Condition::NotOverflow =>  {
            let nof = new_temp(1);
            cg.not_b(&nof,&OF.to_rv());
            fun(&nof,cg)
        },
		Condition::Carry => fun(&*CF,cg),
		Condition::AboveEqual => {
            let ncf = new_temp(1);
            cg.not_b(&ncf,&CF.to_rv());
            fun(&ncf,cg)
        },
        Condition::Equal => fun(&*ZF,cg),
		Condition::NotEqual => {
            let nzf = new_temp(1);
            cg.not_b(&nzf,&ZF.to_rv());
            fun(&nzf,cg)
        },
        Condition::BelowEqual => {
            let zc = new_temp(1);
            cg.or_b(&zc,&ZF.to_rv(),&CF.to_rv());
            fun(&zc,cg)
        },
	    Condition::Above => {
            let zc = new_temp(1);
            cg.or_b(&zc,&ZF.to_rv(),&CF.to_rv());
            cg.not_b(&zc,&zc);
            fun(&zc,cg)
        },
		Condition::Sign => fun(&*SF,cg),
		Condition::NotSign => {
            let nsf = new_temp(1);
            cg.not_b(&nsf,&SF.to_rv());
            fun(&nsf,cg)
        },
        Condition::Parity => fun(&*PF,cg),
		Condition::NotParity => {
		    let npf = new_temp(1);
            cg.not_b(&npf,&PF.to_rv());
            fun(&npf,cg)
        },
        Condition::Less => {
	        let b = new_temp(1);
            cg.xor_b(&b,&SF.to_rv(),&OF.to_rv());
            cg.not_b(&b,&b.to_rv());
            fun(&b,cg)
        },
        Condition::GreaterEqual => {
	        let b = new_temp(1);
            cg.xor_b(&b,&SF.to_rv(),&OF.to_rv());
            fun(&b,cg)
        },
        Condition::LessEqual => {
            let b = new_temp(1);
            cg.xor_b(&b,&SF.to_rv(),&OF.to_rv());
            cg.not_b(&b,&b.to_rv());
            cg.or_b(&b,&b,&ZF.to_rv());
            fun(&b,cg)
        },
        Condition::Greater => {
            let b = new_temp(1);
            let z = new_temp(1);
            cg.xor_b(&b,&SF.to_rv(),&OF.to_rv());
            cg.not_b(&z,&ZF.to_rv());
            cg.or_b(&b,&b,&z.to_rv());
            fun(&b,cg)
        },
    }
}

pub fn cmp(cg: &mut CodeGen<Amd64>, _a: Rvalue, b: Rvalue) {
    let aw = bitwidth(&_a);
    let bw = if let Rvalue::Constant(c) = b { aw } else { bitwidth(&b) };
    let res = new_temp(aw);
    let res_half = new_temp(8);
    let a = Lvalue::from_rvalue(&_a).unwrap();
    let b_ext = if aw == bw { b.clone() } else { sign_ext(&b,bw,aw,cg) };

    cg.sub_i(&res,&a,&b_ext);
    cg.mod_i(&res_half,&res.to_rv(),&Rvalue::Constant(0x100));

	set_arithm_flags(&res,&res_half.to_rv(),&a.to_rv(),&b,cg);
}

pub fn cmps(cg: &mut CodeGen<Amd64>, aoff: Rvalue, boff: Rvalue) {
	let a = Lvalue::Memory{
        offset: Box::new(aoff.clone()),
        bytes: 1,
        endianess: Endianess::Little,
        name: "ram".to_string()
    };
    let b = Lvalue::Memory{
        offset: Box::new(boff.clone()),
        bytes: 1,
        endianess: Endianess::Little,
        name: "ram".to_string()
    };
    let res = new_temp(8);
    let off = new_temp(bitwidth(&aoff));
    let n = new_temp(1);
    let df = new_temp(bitwidth(&aoff));
    let ndf = new_temp(bitwidth(&aoff));

    cg.sub_i(&res,&a.to_rv(),&b.to_rv());
	set_arithm_flags(&res,&res.to_rv(),&a.to_rv(),&b.to_rv(),cg);

    cg.lift_b(&df,&DF.to_rv());
    cg.not_b(&n,&DF.to_rv());
    cg.lift_b(&ndf,&n.to_rv());

    cg.sub_i(&off,&df,&ndf);

    let ao = Lvalue::from_rvalue(&aoff).unwrap();
    let bo = Lvalue::from_rvalue(&boff).unwrap();
    cg.add_i(&ao,&aoff,&off);
    cg.add_i(&bo,&boff,&off);
}

pub fn cmpxchg(cg: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {
    cg.equal_i(&*ZF,&a,&eax.to_rv());

    let n = new_temp(1);
    let zf = new_temp(32);
    let nzf = new_temp(32);
    let la = Lvalue::from_rvalue(&a).unwrap();

    cg.lift_b(&zf,&ZF.to_rv());
    cg.not_b(&n,&ZF.to_rv());
    cg.lift_b(&nzf,&n.to_rv());
    cg.mul_i(&zf,&zf,&b);
    cg.mul_i(&nzf,&nzf,&a);
    cg.add_i(&la,&zf,&nzf);

    cg.lift_b(&zf,&ZF.to_rv());
    cg.lift_b(&nzf,&n.to_rv());
    cg.mul_i(&zf,&zf,&eax.to_rv());
    cg.add_i(&*eax,&zf,&nzf);
}

pub fn or(cg: &mut CodeGen<Amd64>, _a: Rvalue, b: Rvalue) {
    let aw = bitwidth(&_a);
    let bw = if let Rvalue::Constant(c) = b { aw } else { bitwidth(&b) };
    let res = new_temp(aw);
    let res_half = new_temp(8);
    let a = Lvalue::from_rvalue(&_a).unwrap();
    let b_ext = if aw == bw { b.clone() } else { sign_ext(&b,bw,aw,cg) };

    cg.or_i(&res,&a,&b_ext);
    cg.mod_i(&res_half,&res.to_rv(),&Rvalue::Constant(0x100));

    cg.assign(&a,&res.to_rv());
	set_arithm_flags(&res,&res_half.to_rv(),&a.to_rv(),&b,cg);
}

pub fn sbb(cg: &mut CodeGen<Amd64>, _a: Rvalue, b: Rvalue) {
    let aw = bitwidth(&_a);
    let bw = if let Rvalue::Constant(c) = b { aw } else { bitwidth(&b) };
    let res = new_temp(aw);
    let res_half = new_temp(8);
    let a = Lvalue::from_rvalue(&_a).unwrap();
    let b_ext = if aw == bw { b.clone() } else { sign_ext(&b,bw,aw,cg) };

    cg.sub_i(&res,&a,&b_ext);
    cg.sub_i(&res,&res.to_rv(),&CF.to_rv());
    cg.mod_i(&res_half,&res.to_rv(),&Rvalue::Constant(0x100));

    cg.assign(&a,&res.to_rv());
	set_arithm_flags(&res,&res_half.to_rv(),&a.to_rv(),&b,cg);
}

pub fn sub(cg: &mut CodeGen<Amd64>, _a: Rvalue, b: Rvalue) {
    let aw = bitwidth(&_a);
    let bw = if let Rvalue::Constant(c) = b { aw } else { bitwidth(&b) };
    let res = new_temp(aw);
    let res_half = new_temp(8);
    let a = Lvalue::from_rvalue(&_a).unwrap();
    let b_ext = if aw == bw { b.clone() } else { sign_ext(&b,bw,aw,cg) };

    cg.sub_i(&res,&a,&b_ext);
    cg.mod_i(&res_half,&res.to_rv(),&Rvalue::Constant(0x100));

    cg.assign(&a,&res.to_rv());
	set_arithm_flags(&res,&res_half.to_rv(),&a.to_rv(),&b,cg);
}

pub fn xor(cg: &mut CodeGen<Amd64>, _a: Rvalue, b: Rvalue) {
	let aw = bitwidth(&_a);
    let bw = if let Rvalue::Constant(c) = b { aw } else { bitwidth(&b) };
    let res = new_temp(aw);
    let res_half = new_temp(8);
    let a = Lvalue::from_rvalue(&_a).unwrap();
    let b_ext = if aw == bw { b.clone() } else { sign_ext(&b,bw,aw,cg) };

    cg.xor_i(&res,&a,&b_ext);
    cg.mod_i(&res_half,&res.to_rv(),&Rvalue::Constant(0x100));

    cg.assign(&a,&res.to_rv());
	set_arithm_flags(&res,&res_half.to_rv(),&a.to_rv(),&b,cg);
}

pub fn cmpxchg8b(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn cmpxchg16b(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn cpuid(_: &mut CodeGen<Amd64>) {}
pub fn clc(_: &mut CodeGen<Amd64>) {}
pub fn cld(_: &mut CodeGen<Amd64>) {}
pub fn cli(_: &mut CodeGen<Amd64>) {}
pub fn cmc(_: &mut CodeGen<Amd64>) {}
pub fn std(_: &mut CodeGen<Amd64>) {}
pub fn sti(_: &mut CodeGen<Amd64>) {}
pub fn stc(_: &mut CodeGen<Amd64>) {}

pub fn conv(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"conv","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn conv2(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"conv2","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn daa(_: &mut CodeGen<Amd64>) {}
pub fn das(_: &mut CodeGen<Amd64>) {}
pub fn dec(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn div(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn enter(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn hlt(_: &mut CodeGen<Amd64>) {}
pub fn idiv(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn imul1(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn imul2(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn imul3(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue, c: Rvalue) {}
pub fn in_(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn icebp(_: &mut CodeGen<Amd64>) {}
pub fn inc(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn ins(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn int(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn into(_: &mut CodeGen<Amd64>) {}

pub fn iret(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"iret","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn jcc(_: &mut CodeGen<Amd64>,a: Rvalue, c: Condition) {}
pub fn jmp(_: &mut CodeGen<Amd64>,a: Rvalue) {}
pub fn jcxz(_: &mut CodeGen<Amd64>,a: Rvalue) {}
pub fn jecxz(_: &mut CodeGen<Amd64>,a: Rvalue) {}
pub fn jrcxz(_: &mut CodeGen<Amd64>,a: Rvalue) {}
pub fn lahf(_: &mut CodeGen<Amd64>) {}
pub fn lar(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn lds(cg: &mut CodeGen<Amd64>,a: Rvalue, b: Rvalue) { lxs(cg,a,b,ds.to_rv()) }
pub fn les(cg: &mut CodeGen<Amd64>,a: Rvalue, b: Rvalue) { lxs(cg,a,b,es.to_rv()) }
pub fn lss(cg: &mut CodeGen<Amd64>,a: Rvalue, b: Rvalue) { lxs(cg,a,b,ss.to_rv()) }
pub fn lfs(cg: &mut CodeGen<Amd64>,a: Rvalue, b: Rvalue) { lxs(cg,a,b,fs.to_rv()) }
pub fn lgs(cg: &mut CodeGen<Amd64>,a: Rvalue, b: Rvalue) { lxs(cg,a,b,gs.to_rv()) }
pub fn lxs(_: &mut CodeGen<Amd64>,a: Rvalue, b: Rvalue, seg: Rvalue) {}
pub fn lea(_: &mut CodeGen<Amd64>,a: Rvalue, b: Rvalue) {}

pub fn leave(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"leave","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn lodsb(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"lodsb","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn lods(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"lods","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn loop_(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"loop","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn loope(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"loope","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn loopne(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"loopne","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn mov(_: &mut CodeGen<Amd64>,a: Rvalue,b: Rvalue) {}
pub fn movbe(_: &mut CodeGen<Amd64>,a: Rvalue,b: Rvalue) {}

pub fn movsb(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"movsb","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn movs(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"movs","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn movsx(_: &mut CodeGen<Amd64>,a: Rvalue,b: Rvalue) {}
pub fn movzx(_: &mut CodeGen<Amd64>,a: Rvalue,b: Rvalue) {}
pub fn mul(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn neg(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn nop(_: &mut CodeGen<Amd64>) {}
pub fn not(_: &mut CodeGen<Amd64>,a: Rvalue) {}
pub fn out(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}

pub fn outs(cg: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}

pub fn pop(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"pop","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn popa(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"popa","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn popcnt(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn popf(_: &mut CodeGen<Amd64>,a: Rvalue) {}

pub fn push(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"push","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn pusha(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"pusha","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn pushf(_: &mut CodeGen<Amd64>,a: Rvalue) {}
pub fn rcl(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn rcr(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn ret(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn retf(_: &mut CodeGen<Amd64>, a: Rvalue) {}
pub fn ror(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn rol(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn sahf(_: &mut CodeGen<Amd64>) {}
pub fn sal(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn salc(_: &mut CodeGen<Amd64>) {}
pub fn sar(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}

pub fn scas(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"scas","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn setcc(_: &mut CodeGen<Amd64>, a: Rvalue, c: Condition) {}
pub fn shl(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn shr(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn shld(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue, c: Rvalue) {}
pub fn shrd(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue, c: Rvalue) {}

pub fn stos(st: &mut State<Amd64>) -> bool {
    let next = st.address + (st.tokens.len() as u64);
    let len = st.tokens.len();

    st.mnemonic(len,"stos","{}",vec![],&|_: &mut CodeGen<Amd64>| {} );
    st.jump(Rvalue::Constant(next),Guard::always());
    true
}

pub fn test(_: &mut CodeGen<Amd64>,a: Rvalue, b: Rvalue) {}
pub fn ud1(_: &mut CodeGen<Amd64>) {}
pub fn ud2(_: &mut CodeGen<Amd64>) {}
pub fn xadd(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
pub fn xchg(_: &mut CodeGen<Amd64>, a: Rvalue, b: Rvalue) {}
