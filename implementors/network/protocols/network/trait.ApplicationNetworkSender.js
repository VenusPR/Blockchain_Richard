(function() {var implementors = {};
implementors["consensus"] = [{"text":"impl <a class=\"trait\" href=\"network/protocols/network/trait.ApplicationNetworkSender.html\" title=\"trait network::protocols::network::ApplicationNetworkSender\">ApplicationNetworkSender</a>&lt;<a class=\"enum\" href=\"consensus/network_interface/enum.ConsensusMsg.html\" title=\"enum consensus::network_interface::ConsensusMsg\">ConsensusMsg</a>&gt; for <a class=\"struct\" href=\"consensus/network_interface/struct.ConsensusNetworkSender.html\" title=\"struct consensus::network_interface::ConsensusNetworkSender\">ConsensusNetworkSender</a>","synthetic":false,"types":["consensus::network_interface::ConsensusNetworkSender"]}];
implementors["diem_mempool"] = [{"text":"impl <a class=\"trait\" href=\"network/protocols/network/trait.ApplicationNetworkSender.html\" title=\"trait network::protocols::network::ApplicationNetworkSender\">ApplicationNetworkSender</a>&lt;<a class=\"enum\" href=\"diem_mempool/network/enum.MempoolSyncMsg.html\" title=\"enum diem_mempool::network::MempoolSyncMsg\">MempoolSyncMsg</a>&gt; for <a class=\"struct\" href=\"diem_mempool/network/struct.MempoolNetworkSender.html\" title=\"struct diem_mempool::network::MempoolNetworkSender\">MempoolNetworkSender</a>","synthetic":false,"types":["diem_mempool::shared_mempool::network::MempoolNetworkSender"]}];
implementors["network"] = [];
implementors["network_builder"] = [{"text":"impl <a class=\"trait\" href=\"network/protocols/network/trait.ApplicationNetworkSender.html\" title=\"trait network::protocols::network::ApplicationNetworkSender\">ApplicationNetworkSender</a>&lt;<a class=\"struct\" href=\"network_builder/dummy/struct.DummyMsg.html\" title=\"struct network_builder::dummy::DummyMsg\">DummyMsg</a>&gt; for <a class=\"struct\" href=\"network_builder/dummy/struct.DummyNetworkSender.html\" title=\"struct network_builder::dummy::DummyNetworkSender\">DummyNetworkSender</a>","synthetic":false,"types":["network_builder::dummy::DummyNetworkSender"]}];
implementors["state_sync_v1"] = [{"text":"impl <a class=\"trait\" href=\"network/protocols/network/trait.ApplicationNetworkSender.html\" title=\"trait network::protocols::network::ApplicationNetworkSender\">ApplicationNetworkSender</a>&lt;<a class=\"enum\" href=\"state_sync_v1/network/enum.StateSyncMessage.html\" title=\"enum state_sync_v1::network::StateSyncMessage\">StateSyncMessage</a>&gt; for <a class=\"struct\" href=\"state_sync_v1/network/struct.StateSyncSender.html\" title=\"struct state_sync_v1::network::StateSyncSender\">StateSyncSender</a>","synthetic":false,"types":["state_sync_v1::network::StateSyncSender"]}];
implementors["storage_service_client"] = [{"text":"impl <a class=\"trait\" href=\"network/protocols/network/trait.ApplicationNetworkSender.html\" title=\"trait network::protocols::network::ApplicationNetworkSender\">ApplicationNetworkSender</a>&lt;<a class=\"enum\" href=\"storage_service_types/enum.StorageServiceMessage.html\" title=\"enum storage_service_types::StorageServiceMessage\">StorageServiceMessage</a>&gt; for <a class=\"struct\" href=\"storage_service_client/struct.StorageServiceNetworkSender.html\" title=\"struct storage_service_client::StorageServiceNetworkSender\">StorageServiceNetworkSender</a>","synthetic":false,"types":["storage_service_client::StorageServiceNetworkSender"]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()