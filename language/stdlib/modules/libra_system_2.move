address 0x0:

// These modules only change resources stored under the three addresses:
// LibraAssociation's account address: 0xA550C18
// ValidatorSet resource address: 0x1D8
// DiscoverySet resource address: 0xD15C0
// ValidatorSet and DiscoverySet are stored under different addresses in order to facilitate
// cheaper reads of these sets from storage.
module LibraSystem2 {
    use 0x0::LibraConfig;
    use 0x0::Transaction;
    use 0x0::ValidatorConfig2;
    use 0x0::Vector;

    // ValidatorInfo stores consensus config
    // TODO(valerini): consider writing a map module (address -> ValidatorInfo),
    //                 currently implemented with Vector
    struct ValidatorInfo {
        addr: address,
        consensus_voting_power: u64,
        config: ValidatorConfig2::Config,
        // To protect against DDoS, the force updates are allowed only if the time passed
        // since the last force update is larger than a threshold (24hr).
        last_force_update_time: u64,
    }

    struct T {
        // The current consensus crypto scheme.
        scheme: u8,
        // The current validator set. Updated only at epoch boundaries via reconfiguration.
        validators: vector<ValidatorInfo>,
    }

    // ValidatorInfo public accessors

    // Get validator's address from ValidatorInfo
    public fun get_validator_address(v: &ValidatorInfo): address {
        v.addr
    }

    // Get validator's config from ValidatorInfo
    public fun get_validator_config(v: &ValidatorInfo): &ValidatorConfig2::Config {
        &v.config
    }

    // Get validator's voting power from ValidatorInfo
    public fun get_validator_voting_power(v: &ValidatorInfo): &u64 {
        &v.consensus_voting_power
    }

    // Get last forced update time
    public fun get_last_force_update_time(v: &ValidatorInfo): &u64 {
        &v.last_force_update_time
    }

    // This can only be invoked by the ValidatorSet address to instantiate the resource under that address.
    // It can only be called a single time. Currently, it is invoked in the genesis transaction.
    public fun initialize_validator_set() {
        Transaction::assert(Transaction::sender() == 0x1D8, 1);

        LibraConfig::publish_new_config<T>(T {
            scheme: 0,
            validators: Vector::empty(),
        });
    }

    // This returns a copy of the current validator set.
    public fun get_validator_set(): T {
        LibraConfig::get<T>(0x1D8)
    }

    // This copies the vector of validators into the LibraConfig's resource under ValidatorSet address
    fun set_validator_set(value: T) {
        LibraConfig::set<T>(0x1D8, value)
    }

    // Adds a new validator, only callable by the LibraAssociation address
    // TODO(valerini): allow the Association to add multiple validators in a single block
    public fun add_validator(account_address: address) {
        Transaction::assert(Transaction::sender() == 0xA550C18, 1);
        // A prospective validator must have a validator config resource
        Transaction::assert(ValidatorConfig2::has(account_address), 17);

        let validator_set = get_validator_set();
        // Ensure that this address is not already a validator
        Transaction::assert(!is_validator_(account_address, &validator_set.validators), 18);

        let config = ValidatorConfig2::get_config(account_address);
        Vector::push_back(&mut validator_set.validators, ValidatorInfo {
            addr: account_address,
            config: config, // copy the config over to ValidatorSet
            consensus_voting_power: 1,
            last_force_update_time: 0,
        });

        set_validator_set(validator_set);
    }

    // Removes a validator, only callable by the LibraAssociation address
    public fun remove_validator(account_address: address) {
        Transaction::assert(Transaction::sender() == 0xA550C18, 1);
        let validator_set = get_validator_set();
        let size = Vector::length(&validator_set.validators);
        // Make sure the size of validator set does not go beyond 4, which is the minimum safe number of validators
        Transaction::assert(size > 4, 22);

        // Ensure that this address is an active validator
        let to_remove_index_vec = get_validator_index_(&validator_set.validators, account_address);
        Transaction::assert(!Vector::is_empty(&to_remove_index_vec), 21);
        let to_remove_index = *Vector::borrow(&to_remove_index_vec, 0);
        // Remove corresponding ValidatorInfo from the validator set
        _  = Vector::swap_remove(&mut validator_set.validators, to_remove_index);

        set_validator_set(validator_set);
    }

    // Copies validator's config to ValidatorSet
    // Callable by the LibraAssociation address only.
    // Aborts if there are no changes to the config.
    public fun update_validator_info(addr: address) {
        Transaction::assert(Transaction::sender() == 0xA550C18, 1);
        let validator_set = get_validator_set();

        let validator_index_vec = get_validator_index_(&validator_set.validators, addr);
        Transaction::assert(!Vector::is_empty(&validator_index_vec), 19);

        let validator_index = *Vector::borrow(&validator_index_vec, 0);
        if (update_ith_validator_info_(&mut validator_set.validators, validator_index)) {
            set_validator_set(validator_set);
        };
    }

    public fun force_update_validator_info_of_sender() {
        force_update_validator_info(Transaction::sender());
    }

    // This function can be called by the validator's operator,
    // which is either the validator itself or the validator's delegate account.
    // Aborts if there are no changes to the config.
    public fun force_update_validator_info(validator_address: address) {
        // Check the sender
        Transaction::assert(ValidatorConfig2::get_validator_operator_account(validator_address) == Transaction::sender(), 1);

        let validator_set = get_validator_set();
        let validator_index_vec = get_validator_index_(&validator_set.validators, validator_address);
        Transaction::assert(!Vector::is_empty(&validator_index_vec), 19);
        let validator_index = *Vector::borrow(&validator_index_vec, 0);

        // TODO(valerini): add rate-limits for forced key rotations

        // Copy validator config from validator's account and assert that there are changes to the config
        if (update_ith_validator_info_(&mut validator_set.validators, validator_index)) {
            set_validator_set(validator_set);
        };
    }

    // This function can be invoked by the LibraAssociation or by the 0x0 LibraVM address in
    // block_prologue to facilitate reconfigurations at regular intervals.
    // Here for all of the validators the information from ValidatorConfig will get copied into the ValidatorSet,
    // if any components of the configs have changed, appropriate events will be fired.
    public fun update_all_validator_info() {
        Transaction::assert(Transaction::sender() == 0xA550C18 || Transaction::sender() == 0x0, 1);
        let validator_set = get_validator_set();
        let validators = &mut validator_set.validators;

        let size = Vector::length(validators);
        if (size == 0) {
            return
        };

        let i = 0;
        let configs_changed = false;
        configs_changed = configs_changed || update_ith_validator_info_(validators, i);
        loop {
            i = i + 1;
            if (i >= size) { break };
            configs_changed = configs_changed || update_ith_validator_info_(validators, i);
        };
        if (configs_changed) {
            set_validator_set(validator_set);
        };
    }

    // Public getters

    // Return true if addr is a current validator
    public fun is_validator(addr: address): bool { is_validator_(addr, &get_validator_set().validators) }

    // Returns validator info
    // If the address is not a validator, abort
    public fun get_validator_info(addr: address): ValidatorInfo {
        let validator_set = get_validator_set();
        let validator_index_vec = get_validator_index_(&validator_set.validators, addr);
        Transaction::assert(!Vector::is_empty(&validator_index_vec), 19);

        *Vector::borrow(&validator_set.validators, *Vector::borrow(&validator_index_vec, 0))
    }

    // Return the size of the current validator set
    public fun validator_set_size(): u64 {
        Vector::length(&get_validator_set().validators)
    }

    // This function is used in transaction_fee.move to distribute transaction fees among validators
    public fun get_ith_validator_address(i: u64): address {
        let validator_set = get_validator_set();
        let len = Vector::length(&validator_set.validators);
        Transaction::assert(i < len, 3);
        Vector::borrow(&validator_set.validators, i).addr
    }

    // Private helper functions

    // Emit a reconfiguration event. This function will be invoked by the genesis to generate the very first
    // reconfiguration event.
    // Get the index of the validator by address in the `validators` vector
    fun get_validator_index_(validators: &vector<ValidatorInfo>, addr: address): vector<u64> {
        let size = Vector::length(validators);
        let result: vector<u64> = Vector::empty();
        if (size == 0) {
            return result
        };

        let i = 0;
        let validator_info_ref = Vector::borrow(validators, i);
        loop {
            if (validator_info_ref.addr == addr) {
                Vector::push_back(&mut result, i);
                return result
            };
            i = i + 1;
            if (i >= size) { break };
            validator_info_ref = Vector::borrow(validators, i);
        };

        result
    }

    // Updates ith validator info, if nothing changed, return false
    fun update_ith_validator_info_(validators: &mut vector<ValidatorInfo>, i: u64): bool {
        let size = Vector::length(validators);
        if (i >= size) {
            return false
        };
        let validator_info = Vector::borrow_mut(validators, i);
        let new_validator_config = ValidatorConfig2::get_config(validator_info.addr);
        // check if information is the same
        let config_ref = &mut validator_info.config;
        let configs_changed = (config_ref != &new_validator_config);
        *config_ref = *&new_validator_config;
        configs_changed
    }

    fun is_validator_(addr: address, validators_vec_ref: &vector<ValidatorInfo>): bool {
        !Vector::is_empty(&get_validator_index_(validators_vec_ref, addr))
    }
}
