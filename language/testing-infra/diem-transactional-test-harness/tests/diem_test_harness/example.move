//# init --addresses A=0x4777eb94491650dd3f095ce6f778acb6

// To create new accounts for testing, you need to call functions in `mod DiemAccount`
// as Diem Association (or other addresses with required capabilities).
//
// You will need to provide the address of the new account, and the authentication key
// prefix, which can be generated by the tool `diem-keygen`.

//# run --signers 0xA550C18
//#     --private-key 1b8c20cde2dbb43cd3c709b290ac50dcd2be2a87a3a24544b5a5109bc76ea7fb

script {
    use DiemFramework::DiemAccount::create_validator_account;

    fun main(s: signer) {
        create_validator_account(&s, @A, x"f75daa73fc071f93593335eb9033da80", x"40");
    }
}

// In order to get authenticated and run a transaction successfully, you *must* provide
// the correct private key that corresponds to the address and auth key prefix used to
// create the account, as an additional argument to the run command.

//# run --signers 0x4777eb94491650dd3f095ce6f778acb6
//#     --private-key 56a26140eb233750cd14fb168c3eb4bd0782b099cde626ec8aff7f3cceb6364f
script {
    fun main() {}
}


//# publish --private-key 56a26140eb233750cd14fb168c3eb4bd0782b099cde626ec8aff7f3cceb6364f
module A::M {
    public(script) fun foo() {
        abort 42
    }
}


//# run --signers 0x4777eb94491650dd3f095ce6f778acb6
//#     --private-key 56a26140eb233750cd14fb168c3eb4bd0782b099cde626ec8aff7f3cceb6364f
//#     -- 0x4777eb94491650dd3f095ce6f778acb6::M::foo


//# view --address 0x4777eb94491650dd3f095ce6f778acb6 --resource 0x1::DiemAccount::DiemAccount
