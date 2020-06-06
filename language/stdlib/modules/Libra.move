address 0x0 {

module Libra {
    use 0x0::Association;
    use 0x0::Event;
    use 0x0::FixedPoint32::{Self, FixedPoint32};
    use 0x0::LibraConfig;
    use 0x0::RegisteredCurrencies;
    use 0x0::Signer;
    use 0x0::Transaction;
    use 0x0::Vector;

    // The currency has a `CoinType` color that tells us what currency the
    // `value` inside represents.
    resource struct Libra<CoinType> { value: u64 }

    // A minting capability allows coins of type `CoinType` to be minted
    resource struct MintCapability<CoinType> { }

    // A burn capability allows coins of type `CoinType` to be burned
    resource struct BurnCapability<CoinType> { }

    // A operations capability to allow this module to register currencies
    // with the RegisteredCurrencies on-chain config.
    resource struct CurrencyRegistrationCapability {
        cap: RegisteredCurrencies::RegistrationCapability,
    }

    struct MintEvent {
        // funds added to the system
        amount: u64,
        // ASCII encoded symbol for the coin type (e.g., "LBR")
        currency_code: vector<u8>,
    }

    struct BurnEvent {
        // funds removed from the system
        amount: u64,
        // ASCII encoded symbol for the coin type (e.g., "LBR")
        currency_code: vector<u8>,
        // address with the Preburn resource that stored the now-burned funds
        preburn_address: address,
    }

    struct PreburnEvent {
        // funds waiting to be removed from the system
        amount: u64,
        // ASCII encoded symbol for the coin type (e.g., "LBR")
        currency_code: vector<u8>,
        // address with the Preburn resource that now holds the funds
        preburn_address: address,
    }

    struct CancelBurnEvent {
        // funds returned
        amount: u64,
        // ASCII encoded symbol for the coin type (e.g., "LBR")
        currency_code: vector<u8>,
        // address with the Preburn resource that holds the now-returned funds
        preburn_address: address,
    }

    // The information for every supported currency is stored in a resource
    // under the `currency_addr()` address. Unless they are specified
    // otherwise the fields in this resource are immutable.
    resource struct CurrencyInfo<CoinType> {
        // The total value for the currency represented by
        // `CoinType`. Mutable.
        total_value: u128,
        // Value of funds that are in the process of being burned
        preburn_value: u64,
        // The (rough) exchange rate from `CoinType` to LBR.
        to_lbr_exchange_rate: FixedPoint32,
        // Holds whether or not this currency is synthetic (contributes to the
        // off-chain reserve) or not. An example of such a currency would be
        // the LBR.
        is_synthetic: bool,
        // The scaling factor for the coin (i.e. the amount to multiply by
        // to get to the human-readable reprentation for this currency). e.g. 10^6 for Coin1
        scaling_factor: u64,
        // The smallest fractional part (number of decimal places) to be
        // used in the human-readable representation for the currency (e.g.
        // 10^2 for Coin1 cents)
        fractional_part: u64,
        // The code symbol for this `CoinType`. ASCII encoded.
        // e.g. for "LBR" this is x"4C4252". No character limit.
        currency_code: vector<u8>,
        // We may want to disable the ability to mint further coins of a
        // currency while that currency is still around. Mutable.
        can_mint: bool,
        // event stream for minting
        mint_events: Event::EventHandle<MintEvent>,
        // event stream for burning
        burn_events: Event::EventHandle<BurnEvent>,
        // event stream for preburn requests
        preburn_events: Event::EventHandle<PreburnEvent>,
        // event stream for cancelled preburn requests
        cancel_burn_events: Event::EventHandle<CancelBurnEvent>,
    }

    // A holding area where funds that will subsequently be burned wait while their underyling
    // assets are sold off-chain.
    // This resource can only be created by the holder of the BurnCapability. An account that
    // contains this address has the authority to initiate a burn request. A burn request can be
    // resolved by the holder of the BurnCapability by either (1) burning the funds, or (2)
    // returning the funds to the account that initiated the burn request.
    // This design supports multiple preburn requests in flight at the same time, including multiple
    // burn requests from the same account. However, burn requests from the same account must be
    // resolved in FIFO order.
    resource struct Preburn<Token> {
        // Queue of pending burn requests
        requests: vector<Libra<Token>>,
    }

    // An association account holding this privilege can add/remove the
    // currencies from the system.
    struct AddCurrency { }

    ///////////////////////////////////////////////////////////////////////////
    // Initialization and granting of privileges
    ///////////////////////////////////////////////////////////////////////////

    // This is only invoked in the genesis transaction
    public fun initialize(config_account: &signer) {
        Transaction::assert(
            Signer::address_of(config_account) == LibraConfig::default_config_address(),
            0
        );
        let cap = RegisteredCurrencies::initialize(config_account);
        move_to(config_account, CurrencyRegistrationCapability{ cap })
    }


    // TODO: temporary, we should ideally make MintCapability unique eventually...
    public fun grant_mint_capability_to_association<CoinType>(association: &signer) {
        assert_assoc_and_currency<CoinType>(association);
        move_to(association, MintCapability<CoinType>{})
    }

    // Publish the `MintCapability` `cap` for the `CoinType` currency under `account`. `CoinType`
    // must be a registered currency type.
    public fun publish_mint_capability<CoinType>(account: &signer, cap: MintCapability<CoinType>) {
        assert_assoc_and_currency<CoinType>(account);
        move_to(account, cap)
    }

    // Publish the `BurnCapability` `cap` for the `CoinType` currency under `account`. `CoinType`
    // must be a registered currency type.
    public fun publish_burn_capability<CoinType>(account: &signer, cap: BurnCapability<CoinType>) {
        assert_assoc_and_currency<CoinType>(account);
        move_to(account, cap)
    }

    // Return `amount` coins.
    // Fails if the sender does not have a published MintCapability.
    public fun mint<Token>(account: &signer, amount: u64): Libra<Token>
    acquires CurrencyInfo, MintCapability {
        mint_with_capability(
            amount,
            borrow_global<MintCapability<Token>>(Signer::address_of(account))
        )
    }

    // Burn the coins currently held in the preburn holding area under `preburn_address`.
    // Fails if the sender does not have a published `BurnCapability`.
    public fun burn<Token>(
        account: &signer,
        preburn_address: address
    ) acquires BurnCapability, CurrencyInfo, Preburn {
        burn_with_capability(
            preburn_address,
            borrow_global<BurnCapability<Token>>(Signer::address_of(account))
        )
    }

    // Cancel the oldest burn request from `preburn_address`
    // Fails if the sender does not have a published `BurnCapability`.
    public fun cancel_burn<Token>(
        account: &signer,
        preburn_address: address
    ): Libra<Token> acquires BurnCapability, CurrencyInfo, Preburn {
        cancel_burn_with_capability(
            preburn_address,
            borrow_global<BurnCapability<Token>>(Signer::address_of(account))
        )
    }

    public fun new_preburn<Token>(): Preburn<Token> {
        assert_is_coin<Token>();
        Preburn<Token> { requests: Vector::empty() }
    }

    // Mint a new Libra worth `value`. The caller must have a reference to a MintCapability.
    // Only the Association account can acquire such a reference, and it can do so only via
    // `borrow_sender_mint_capability`
    public fun mint_with_capability<Token>(
        value: u64,
        _capability: &MintCapability<Token>
    ): Libra<Token> acquires CurrencyInfo {
        assert_is_coin<Token>();
        // TODO: temporary measure for testnet only: limit minting to 1B Libra at a time.
        // this is to prevent the market cap's total value from hitting u64_max due to excessive
        // minting. This will not be a problem in the production Libra system because coins will
        // be backed with real-world assets, and thus minting will be correspondingly rarer.
        // * 1000000 here because the unit is microlibra
        Transaction::assert(value <= 1000000000 * 1000000, 11);
        let currency_code = currency_code<Token>();
        // update market cap resource to reflect minting
        let info = borrow_global_mut<CurrencyInfo<Token>>(0xA550C18);
        Transaction::assert(info.can_mint, 4);
        info.total_value = info.total_value + (value as u128);
        // don't emit mint events for synthetic currenices
        if (!info.is_synthetic) {
            Event::emit_event(
                &mut info.mint_events,
                MintEvent{
                    amount: value,
                    currency_code,
                }
            );
        };

        Libra<Token> { value }
    }

    // Create a new Preburn resource.
    // Can only be called by the holder of the BurnCapability.
    public fun new_preburn_with_capability<Token>(
        _capability: &BurnCapability<Token>
    ): Preburn<Token> {
        assert_is_coin<Token>();
        Preburn<Token> { requests: Vector::empty() }
    }

    // Send a coin to the preburn holding area `preburn` that is passed in.
    public fun preburn_with_resource<Token>(
        coin: Libra<Token>,
        preburn: &mut Preburn<Token>,
        preburn_address: address,
    ) acquires CurrencyInfo {
        let coin_value = value(&coin);
        Vector::push_back(
            &mut preburn.requests,
            coin
        );
        let currency_code = currency_code<Token>();
        let info = borrow_global_mut<CurrencyInfo<Token>>(0xA550C18);
        info.preburn_value = info.preburn_value + coin_value;
        // don't emit preburn events for synthetic currencies
        if (!info.is_synthetic) {
            Event::emit_event(
                &mut info.preburn_events,
                PreburnEvent{
                    amount: coin_value,
                    currency_code,
                    preburn_address,
                }
            );
        };
    }

    ///////////////////////////////////////////////////////////////////////////
    // Treasury Compliance specific methods for DDs
    ///////////////////////////////////////////////////////////////////////////

    // Publish `preburn` under 'account'. Used for bootstrapping designated dealer
    // as assocation TC account is creating this resource for DD
    public fun publish_preburn_to_account<Token>(creator: &signer, account: &signer) {
        Association::assert_account_is_blessed(creator);
        let preburn = Preburn<Token> { requests: Vector::empty() };
        publish_preburn<Token>(account, preburn)
    }

    ///////////////////////////////////////////////////////////////////////////

    /// Send coin to the preburn holding area for `account`, where it will wait to either be burned
    /// or returned to the balance of `account`.
    /// Fails if `account` does not have a published Preburn resource
    public fun preburn_to<Token>(account: &signer, coin: Libra<Token>) acquires CurrencyInfo, Preburn {
        let sender = Signer::address_of(account);
        preburn_with_resource(coin, borrow_global_mut<Preburn<Token>>(sender), sender);
    }

    // Permanently remove the coins held in the `Preburn` resource stored at `preburn_address` and
    // update the market cap accordingly. If there are multiple preburn requests in progress, this
    // will remove the oldest one.
    // Can only be invoked by the holder of the `BurnCapability`. Fails if the there is no `Preburn`
    // resource under `preburn_address` or has one with no pending burn requests.
    public fun burn_with_capability<Token>(
        preburn_address: address,
        capability: &BurnCapability<Token>
    ) acquires CurrencyInfo, Preburn {
        // destroy the coin at the head of the preburn queue
        burn_with_resource_cap(
            borrow_global_mut<Preburn<Token>>(preburn_address),
            preburn_address,
            capability
        )
    }

    // Permanently remove the coins held in the passed-in preburn resource
    // and update the market cap accordingly. If there are multiple preburn
    // requests in progress, this will remove the oldest one.
    // Can only be invoked by the holder of the `BurnCapability`. Fails if
    // the `preburn` resource has no pending burn requests.
    public fun burn_with_resource_cap<Token>(
        preburn: &mut Preburn<Token>,
        preburn_address: address,
        _capability: &BurnCapability<Token>
    ) acquires CurrencyInfo {
        // destroy the coin at the head of the preburn queue
        let Libra { value } = Vector::remove(&mut preburn.requests, 0);
        // update the market cap
        let currency_code = currency_code<Token>();
        let info = borrow_global_mut<CurrencyInfo<Token>>(0xA550C18);
        info.total_value = info.total_value - (value as u128);
        info.preburn_value = info.preburn_value - value;
        // don't emit burn events for synthetic currencies
        if (!info.is_synthetic) {
            Event::emit_event(
                &mut info.burn_events,
                BurnEvent {
                    amount: value,
                    currency_code,
                    preburn_address,
                }
            );
        };
    }

    // Cancel the burn request in the `Preburn` resource stored at `preburn_address` and
    // return the coins to the caller.
    // If there are multiple preburn requests in progress, this will cancel the oldest one.
    // Can only be invoked by the holder of the `BurnCapability`. Fails if the transaction sender
    // does not have a published Preburn resource or has one with no pending burn requests.
    public fun cancel_burn_with_capability<Token>(
        preburn_address: address,
        _capability: &BurnCapability<Token>
    ): Libra<Token> acquires CurrencyInfo, Preburn {
        // destroy the coin at the head of the preburn queue
        let preburn = borrow_global_mut<Preburn<Token>>(preburn_address);
        let coin = Vector::remove(&mut preburn.requests, 0);
        // update the market cap
        let currency_code = currency_code<Token>();
        let info = borrow_global_mut<CurrencyInfo<Token>>(0xA550C18);
        let amount = value(&coin);
        info.preburn_value = info.preburn_value - amount;
        // Don't emit cancel burn events for synthetic currencies. cancel burn shouldn't be be used
        // for synthetics in the first place
        if (!info.is_synthetic) {
            Event::emit_event(
                &mut info.cancel_burn_events,
                CancelBurnEvent {
                    amount,
                    currency_code,
                    preburn_address,
                }
            );
        };

        coin
    }

    // Publish `preburn` under the sender's account
    public fun publish_preburn<Token>(account: &signer, preburn: Preburn<Token>) {
        move_to(account, preburn)
    }


    // Remove and return the `Preburn` resource under the sender's account
    public fun remove_preburn<Token>(account: &signer): Preburn<Token> acquires Preburn {
        move_from<Preburn<Token>>(Signer::address_of(account))
    }

    // Destroys the given preburn resource.
    // Aborts if `requests` is non-empty
    public fun destroy_preburn<Token>(preburn: Preburn<Token>) {
        let Preburn { requests } = preburn;
        Vector::destroy_empty(requests)
    }

    // Remove and return the MintCapability from the sender's account. Fails if the sender does
    // not have a published MintCapability
    public fun remove_mint_capability<Token>(account: &signer): MintCapability<Token>
    acquires MintCapability {
        move_from<MintCapability<Token>>(Signer::address_of(account))
    }

    // Remove and return the BurnCapability from the sender's account. Fails if the sender does
    // not have a published BurnCapability
    public fun remove_burn_capability<Token>(account: &signer): BurnCapability<Token>
    acquires BurnCapability {
        move_from<BurnCapability<Token>>(Signer::address_of(account))
    }

    // Return the total value of Libra to be burned
    public fun preburn_value<Token>(): u64 acquires CurrencyInfo {
        borrow_global<CurrencyInfo<Token>>(0xA550C18).preburn_value
    }

    // Create a new Libra<CoinType> with a value of 0
    public fun zero<CoinType>(): Libra<CoinType> {
        assert_is_coin<CoinType>();
        Libra<CoinType> { value: 0 }
    }

    // Public accessor for the value of a coin
    public fun value<CoinType>(coin: &Libra<CoinType>): u64 {
        coin.value
    }

    // Splits the given coin into two and returns them both
    // It leverages `Self::withdraw` for any verifications of the values
    public fun split<CoinType>(coin: Libra<CoinType>, amount: u64): (Libra<CoinType>, Libra<CoinType>) {
        let other = withdraw(&mut coin, amount);
        (coin, other)
    }

    // "Divides" the given coin into two, where the original coin is modified in place
    // The original coin will have value = original value - `amount`
    // The new coin will have a value = `amount`
    // Fails if the coins value is less than `amount`
    public fun withdraw<CoinType>(coin: &mut Libra<CoinType>, amount: u64): Libra<CoinType> {
        // Check that `amount` is less than the coin's value
        Transaction::assert(coin.value >= amount, 10);
        coin.value = coin.value - amount;
        Libra { value: amount }
    }

    // Merges two coins of the same currency and returns a new coin whose
    // value is equal to the sum of the two inputs
    public fun join<CoinType>(coin1: Libra<CoinType>, coin2: Libra<CoinType>): Libra<CoinType>  {
        deposit(&mut coin1, coin2);
        coin1
    }

    // "Merges" the two coins
    // The coin passed in by reference will have a value equal to the sum of the two coins
    // The `check` coin is consumed in the process
    public fun deposit<CoinType>(coin: &mut Libra<CoinType>, check: Libra<CoinType>) {
        let Libra { value } = check;
        coin.value = coin.value + value;
    }

    // Destroy a coin
    // Fails if the value is non-zero
    // The amount of LibraCoin.T in the system is a tightly controlled property,
    // so you cannot "burn" any non-zero amount of LibraCoin.T
    public fun destroy_zero<CoinType>(coin: Libra<CoinType>) {
        let Libra { value } = coin;
        Transaction::assert(value == 0, 5)
    }

    ///////////////////////////////////////////////////////////////////////////
    // Definition of Currencies
    ///////////////////////////////////////////////////////////////////////////

    // Register the type `CoinType` as a currency. Without this, a type
    // cannot be used as a coin/currency unit n Libra.
    public fun register_currency<CoinType>(
        account: &signer,
        to_lbr_exchange_rate: FixedPoint32,
        is_synthetic: bool,
        scaling_factor: u64,
        fractional_part: u64,
        currency_code: vector<u8>,
    ): (MintCapability<CoinType>, BurnCapability<CoinType>)
    acquires CurrencyRegistrationCapability {
        // And only callable by the designated currency address.
        Transaction::assert(
            Association::has_privilege<AddCurrency>(Signer::address_of(account)),
            8
        );

        move_to(account, CurrencyInfo<CoinType> {
            total_value: 0,
            preburn_value: 0,
            to_lbr_exchange_rate,
            is_synthetic,
            scaling_factor,
            fractional_part,
            currency_code: copy currency_code,
            can_mint: true,
            mint_events: Event::new_event_handle<MintEvent>(account),
            burn_events: Event::new_event_handle<BurnEvent>(account),
            preburn_events: Event::new_event_handle<PreburnEvent>(account),
            cancel_burn_events: Event::new_event_handle<CancelBurnEvent>(account)
        });
        RegisteredCurrencies::add_currency_code(
            currency_code,
            &borrow_global<CurrencyRegistrationCapability>(LibraConfig::default_config_address()).cap
        );
        (MintCapability<CoinType>{}, BurnCapability<CoinType>{})
    }

    // Return the total amount of currency minted of type `CoinType`
    public fun market_cap<CoinType>(): u128
    acquires CurrencyInfo {
        borrow_global<CurrencyInfo<CoinType>>(currency_addr()).total_value
    }

    // Returns the value of the coin in the `FromCoinType` currency in LBR.
    // This should only be used where a _rough_ approximation of the exchange
    // rate is needed.
    public fun approx_lbr_for_value<FromCoinType>(from_value: u64): u64
    acquires CurrencyInfo {
        let lbr_exchange_rate = lbr_exchange_rate<FromCoinType>();
        FixedPoint32::multiply_u64(from_value, lbr_exchange_rate)
    }

    // Returns the value of the coin in the `FromCoinType` currency in LBR.
    // This should only be used where a rough approximation of the exchange
    // rate is needed.
    public fun approx_lbr_for_coin<FromCoinType>(coin: &Libra<FromCoinType>): u64
    acquires CurrencyInfo {
        let from_value = value(coin);
        approx_lbr_for_value<FromCoinType>(from_value)
    }

    // Return true if the type `CoinType` is a registered currency.
    public fun is_currency<CoinType>(): bool {
        exists<CurrencyInfo<CoinType>>(currency_addr())
    }

    // Predicate on whether `CoinType` is a synthetic currency.
    public fun is_synthetic_currency<CoinType>(): bool
    acquires CurrencyInfo {
        let addr = currency_addr();
        exists<CurrencyInfo<CoinType>>(addr) &&
            borrow_global<CurrencyInfo<CoinType>>(addr).is_synthetic
    }

    // Returns the scaling factor for the `CoinType` currency.
    public fun scaling_factor<CoinType>(): u64
    acquires CurrencyInfo {
        borrow_global<CurrencyInfo<CoinType>>(currency_addr()).scaling_factor
    }

    // Returns the representable fractional part for the `CoinType` currency.
    public fun fractional_part<CoinType>(): u64
    acquires CurrencyInfo {
        borrow_global<CurrencyInfo<CoinType>>(currency_addr()).fractional_part
    }

    // Return the currency code for the registered currency.
    public fun currency_code<CoinType>(): vector<u8>
    acquires CurrencyInfo {
        *&borrow_global<CurrencyInfo<CoinType>>(currency_addr()).currency_code
    }

    // Updates the exchange rate for `FromCoinType` to LBR exchange rate held on chain.
    public fun update_lbr_exchange_rate<FromCoinType>(
        account: &signer,
        lbr_exchange_rate: FixedPoint32
    ) acquires CurrencyInfo {
        assert_assoc_and_currency<FromCoinType>(account);
        let currency_info = borrow_global_mut<CurrencyInfo<FromCoinType>>(currency_addr());
        currency_info.to_lbr_exchange_rate = lbr_exchange_rate;
    }

    // Return the (rough) exchange rate between `CoinType` and LBR
    public fun lbr_exchange_rate<CoinType>(): FixedPoint32
    acquires CurrencyInfo {
        *&borrow_global<CurrencyInfo<CoinType>>(currency_addr()).to_lbr_exchange_rate
    }

    // There may be situations in which we disallow the further minting of
    // coins in the system without removing the currency. This function
    // allows the association to control whether or not further coins of
    // `CoinType` can be minted or not.
    public fun update_minting_ability<CoinType>(account: &signer, can_mint: bool)
    acquires CurrencyInfo {
        assert_assoc_and_currency<CoinType>(account);
        let currency_info = borrow_global_mut<CurrencyInfo<CoinType>>(currency_addr());
        currency_info.can_mint = can_mint;
    }

    ///////////////////////////////////////////////////////////////////////////
    // Helper functions
    ///////////////////////////////////////////////////////////////////////////

    // The (singleton) address under which the currency registration
    // information is published.
    fun currency_addr(): address {
        0xA550C18
    }

    // Assert that the sender is an association account, and that
    // `CoinType` is a regstered currency type.
    fun assert_assoc_and_currency<CoinType>(account: &signer) {
        Association::assert_is_association(account);
        assert_is_coin<CoinType>();
    }

    // Assert that `CoinType` is a registered currency
    fun assert_is_coin<CoinType>() {
        Transaction::assert(is_currency<CoinType>(), 1);
    }

    // **************** SPECIFICATIONS ****************
    // Only a few of the specifications appear at this time. More to come.

    // Verify this module
    spec module {

        // Verification is disabled because of an invariant in association.move that
        // causes a violated precondition here.  And "invariant module" in association.move
        // gets an error for some reason.

        pragma verify = false;
    }

    // ## Currency registration

    spec module {
        // Address at which currencies should be registered (mirrors currency_addr)
        define spec_currency_addr(): address { 0xA550C18 }

        // Checks whether currency is registered.
        // Mirrors is_currency<CoinType> in Move, above.
        define spec_is_currency<CoinType>(): bool {
            exists<CurrencyInfo<CoinType>>(spec_currency_addr())
        }
    }

    // Sanity check -- after register_currency is called, currency should be registered.
    spec fun register_currency {
        // This doesn't verify because:
        //  1. is_registered assumes the currency is registered at the fixed
        //     currency_addr()  (0xA550C18).
        //  2. The address where the CurrencyInfo<CoinType>> is stored is
        //     determined in Association::initialize()
        //     (address of AddCurrency privilege) and
        //     Genesis::initialize_association(association_root_addr).
        // If the AddCurrency privilege is on an address different from
        // currency_addr(), the currency will appear not to be registered.
        // If you change next to "true", prover will report an error.
        pragma verify = false;

        // SPEC: After register_currency, the currency is an official currency.
        ensures spec_is_currency<CoinType>();
    }

    spec fun is_currency {
        ensures result == spec_is_currency<CoinType>();
    }

    // Move code
    spec fun assert_is_coin {
        aborts_if !spec_is_currency<CoinType>();
    }

    // Maintain a ghost variable representing the sum of
    // all coins of a currency type.
    spec module {
        global sum_of_coin_values<CoinType>: num;
    }
    spec struct Libra {
        invariant pack sum_of_coin_values<CoinType> = sum_of_coin_values<CoinType> + value;
        invariant unpack sum_of_coin_values<CoinType> = sum_of_coin_values<CoinType> - value;
    }

    // TODO: What happens to the CurrencyRegistrationCapability?


}
}
