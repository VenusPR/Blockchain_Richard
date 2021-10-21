#[test_only]
module 0x1::MultiTokenTests {
    use Std::GUID;
    use 0x1::MultiToken;
    use 0x1::MultiTokenBalance;
    use Std::Option;

    /// A test token type to instantiate generic Tokens with.
    struct Game has store {
        name: vector<u8>,
        edition: u64,
    }

    struct Collection has store {
        name: vector<u8>,
    }

    struct Pokemon has store {
        name: vector<u8>,
        type: vector<u8>,
    }

    const EMINT_FAILED: u64 = 0;
    const ETRANSFER_FAILED: u64 = 1;
    const ECOLLECTION_FAILED: u64 = 2;

    #[test(admin=@0xa550c18, creator=@0x42, user=@0x43)]
    public(script) fun test_all(admin: signer, creator: signer, user: signer) {
        /*
        ===============================================================
            Initialization + preparation
        ===============================================================
        */

        let creator_addr = @0x42;
        let user_addr = @0x43;

        MultiToken::initialize_multi_token(admin);
        MultiTokenBalance::publish_balance<Game>(&creator);
        MultiTokenBalance::publish_balance<Collection>(&creator);
        MultiTokenBalance::publish_balance<Pokemon>(&creator);
        MultiTokenBalance::publish_balance<Game>(&user);

        let token1_id = GUID::create_id(creator_addr, 0);
        let token2_id = GUID::create_id(creator_addr, 1);

        /*
        ===============================================================
            Test minting
        ===============================================================
        */

        let token1 = MultiToken::create<Game>(
            &creator,
            Game { name: b"Mario", edition: 2008 },
            b"nintendo.com",
            10,
            Option::none(),
        );
        // Add all 10 tokens to creator's own account
        MultiTokenBalance::add_to_gallery<Game>(creator_addr, token1);

        // Assert creator has the right number of tokens and supply is 10.
        assert(MultiTokenBalance::has_token<Game>(creator_addr, &token1_id), EMINT_FAILED);
        assert(MultiTokenBalance::get_token_balance<Game>(creator_addr, &token1_id) == 10, EMINT_FAILED);
        assert(MultiToken::supply<Game>(&token1_id) == 10, EMINT_FAILED);

        let token2 = MultiToken::create<Game>(
            &creator,
            Game { name: b"ChromeDino", edition: 2015 },
            b"google.com",
            233,
            Option::none(),
        );
        MultiTokenBalance::add_to_gallery<Game>(creator_addr, token2);
        assert(MultiTokenBalance::has_token<Game>(creator_addr, &token2_id), EMINT_FAILED);
        assert(MultiTokenBalance::get_token_balance<Game>(creator_addr, &token2_id) == 233, EMINT_FAILED);

        /*
        ===============================================================
            Test collections
        ===============================================================
        */

        // Create collection first
        let collection = MultiToken::create<Collection>(
            &creator,
            Collection { name: b"Pokemon" },
            b"nintendo.com",
            1,
            Option::none(),
        );

        let pikachu = MultiToken::create<Pokemon>(
            &creator,
            Pokemon { name: b"Pikachu", type: b"electric", },
            b"nintendo.com",
            10,
            Option::some(MultiToken::id(&collection)),
        );
        let charmander = MultiToken::create<Pokemon>(
            &creator,
            Pokemon { name: b"Charmander", type: b"fire", },
            b"nintendo.com",
            10,
            Option::some(MultiToken::id(&collection)),
        );
        let pikachu_token = MultiToken::extract_token<Pokemon>(&pikachu);
        assert(MultiToken::parent(&pikachu_token) == &Option::some(MultiToken::id(&collection)), ECOLLECTION_FAILED);
        MultiToken::restore_token(pikachu_token);
        MultiTokenBalance::add_to_gallery<Pokemon>(creator_addr, pikachu);

        let charmander_token = MultiToken::extract_token<Pokemon>(&charmander);
        assert(MultiToken::parent(&charmander_token) == &Option::some(MultiToken::id(&collection)), ECOLLECTION_FAILED);
        MultiToken::restore_token(charmander_token);
        MultiTokenBalance::add_to_gallery<Pokemon>(creator_addr, charmander);
        MultiTokenBalance::add_to_gallery<Collection>(creator_addr, collection);

        /*
        ===============================================================
            Test transferring tokens without splitting of tokens
        ===============================================================
        */

        // Transfer 6 units of token1 from creator to user
        MultiTokenBalance::transfer_multi_token_between_galleries<Game>(
            creator, // from
            user_addr, // to
            6, // amount
            creator_addr, // token.id.addr
            0, // token.id.creation_num
        );

        assert(MultiTokenBalance::has_token<Game>(creator_addr, &token1_id), ETRANSFER_FAILED);
        assert(MultiTokenBalance::get_token_balance<Game>(creator_addr, &token1_id) == 4, ETRANSFER_FAILED);
        assert(MultiTokenBalance::has_token<Game>(user_addr, &token1_id), ETRANSFER_FAILED);
        assert(MultiTokenBalance::get_token_balance<Game>(user_addr, &token1_id) == 6, ETRANSFER_FAILED);
        assert(MultiToken::supply<Game>(&token1_id) == 10, ETRANSFER_FAILED); // supply should not change


        /*
        ===============================================================
            Test transferring tokens with splitting of tokens
        ===============================================================
        */

        // Transfer all 6 units of token1 from user to creator
        MultiTokenBalance::transfer_multi_token_between_galleries<Game>(
            user, creator_addr, 6, creator_addr, 0,
        );
        assert(!MultiTokenBalance::has_token<Game>(user_addr, &token1_id), ETRANSFER_FAILED); // user doesn't have token1 anymore
        assert(MultiTokenBalance::get_token_balance<Game>(user_addr, &token1_id) == 0, ETRANSFER_FAILED);
        assert(MultiTokenBalance::has_token<Game>(creator_addr, &token1_id), ETRANSFER_FAILED);
        assert(MultiTokenBalance::get_token_balance<Game>(creator_addr, &token1_id) == 10, ETRANSFER_FAILED);
    }
}
