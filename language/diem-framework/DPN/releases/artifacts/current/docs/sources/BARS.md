
<a name="0x1_BARSToken"></a>

# Module `0x1::BARSToken`



-  [Struct `BARSToken`](#0x1_BARSToken_BARSToken)
-  [Function `register_user`](#0x1_BARSToken_register_user)
-  [Function `register_user_internal`](#0x1_BARSToken_register_user_internal)
-  [Function `mint_bars`](#0x1_BARSToken_mint_bars)
-  [Function `mint_internal`](#0x1_BARSToken_mint_internal)


<pre><code><b>use</b> <a href="../../../../../../../move-stdlib/docs/GUID.md#0x1_GUID">0x1::GUID</a>;
<b>use</b> <a href="NFT.md#0x1_NFT">0x1::NFT</a>;
<b>use</b> <a href="NFTGallery.md#0x1_NFTGallery">0x1::NFTGallery</a>;
<b>use</b> <a href="../../../../../../../move-stdlib/docs/Option.md#0x1_Option">0x1::Option</a>;
<b>use</b> <a href="../../../../../../../move-stdlib/docs/Signer.md#0x1_Signer">0x1::Signer</a>;
</code></pre>



<a name="0x1_BARSToken_BARSToken"></a>

## Struct `BARSToken`



<pre><code><b>struct</b> <a href="BARS.md#0x1_BARSToken">BARSToken</a> has store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>artist_name: vector&lt;u8&gt;</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="0x1_BARSToken_register_user"></a>

## Function `register_user`

Call this function to set up relevant resources in order to
mint and receive tokens.


<pre><code><b>public</b>(<b>script</b>) <b>fun</b> <a href="BARS.md#0x1_BARSToken_register_user">register_user</a>(user: signer)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(<b>script</b>) <b>fun</b> <a href="BARS.md#0x1_BARSToken_register_user">register_user</a>(user: signer) {
    <a href="BARS.md#0x1_BARSToken_register_user_internal">register_user_internal</a>(&user);
}
</code></pre>



</details>

<a name="0x1_BARSToken_register_user_internal"></a>

## Function `register_user_internal`

Need this internal function for testing, since the script fun version
consumes a signer


<pre><code><b>fun</b> <a href="BARS.md#0x1_BARSToken_register_user_internal">register_user_internal</a>(user: &signer)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="BARS.md#0x1_BARSToken_register_user_internal">register_user_internal</a>(user: &signer) {
    // publish TokenBalance&lt;<a href="BARS.md#0x1_BARSToken">BARSToken</a>&gt; <b>resource</b>
    <a href="NFTGallery.md#0x1_NFTGallery_publish_gallery">NFTGallery::publish_gallery</a>&lt;<a href="BARS.md#0x1_BARSToken">BARSToken</a>&gt;(user);

    // publish TokenDataCollection&lt;<a href="BARS.md#0x1_BARSToken">BARSToken</a>&gt; <b>resource</b>
    <a href="NFT.md#0x1_NFT_publish_token_data_collection">NFT::publish_token_data_collection</a>&lt;<a href="BARS.md#0x1_BARSToken">BARSToken</a>&gt;(user);
}
</code></pre>



</details>

<a name="0x1_BARSToken_mint_bars"></a>

## Function `mint_bars`

Mint <code>amount</code> copies of BARS tokens to the artist's account.


<pre><code><b>public</b>(<b>script</b>) <b>fun</b> <a href="BARS.md#0x1_BARSToken_mint_bars">mint_bars</a>(artist: signer, artist_name: vector&lt;u8&gt;, content_uri: vector&lt;u8&gt;, amount: u64)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(<b>script</b>) <b>fun</b> <a href="BARS.md#0x1_BARSToken_mint_bars">mint_bars</a>(
    artist: signer,
    artist_name: vector&lt;u8&gt;,
    content_uri: vector&lt;u8&gt;,
    amount: u64
) {
    <a href="BARS.md#0x1_BARSToken_mint_internal">mint_internal</a>(&artist, artist_name, content_uri, amount);
}
</code></pre>



</details>

<a name="0x1_BARSToken_mint_internal"></a>

## Function `mint_internal`

Need this internal function for testing, since the script fun version
consumes a signer


<pre><code><b>fun</b> <a href="BARS.md#0x1_BARSToken_mint_internal">mint_internal</a>(artist: &signer, artist_name: vector&lt;u8&gt;, content_uri: vector&lt;u8&gt;, amount: u64)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="BARS.md#0x1_BARSToken_mint_internal">mint_internal</a>(
    artist: &signer,
    artist_name: vector&lt;u8&gt;,
    content_uri: vector&lt;u8&gt;,
    amount: u64
) {
    <b>let</b> token = <a href="NFT.md#0x1_NFT_create">NFT::create</a>&lt;<a href="BARS.md#0x1_BARSToken">BARSToken</a>&gt;(
        artist,
        <a href="BARS.md#0x1_BARSToken">BARSToken</a> { artist_name },
        content_uri,
        amount,
        <a href="../../../../../../../move-stdlib/docs/Option.md#0x1_Option_none">Option::none</a>(),
    );
    <a href="NFTGallery.md#0x1_NFTGallery_add_to_gallery">NFTGallery::add_to_gallery</a>(<a href="../../../../../../../move-stdlib/docs/Signer.md#0x1_Signer_address_of">Signer::address_of</a>(artist), token);
}
</code></pre>



</details>


[//]: # ("File containing references which can be used from documentation")
[ACCESS_CONTROL]: https://github.com/diem/dip/blob/main/dips/dip-2.md
[ROLE]: https://github.com/diem/dip/blob/main/dips/dip-2.md#roles
[PERMISSION]: https://github.com/diem/dip/blob/main/dips/dip-2.md#permissions
