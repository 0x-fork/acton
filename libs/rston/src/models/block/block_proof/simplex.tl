// Simplex finalization types from TON's tl/generate/scheme/ton_api.tl.
tonNode.blockIdExt workchain:int shard:long seqno:int root_hash:int256 file_hash:int256 = tonNode.BlockIdExt;
consensus.dataToSign session_id:int256 data:bytes = consensus.DataToSign;
consensus.candidateId slot:int hash:int256 = consensus.CandidateId;
consensus.candidateParent id:consensus.CandidateId = consensus.CandidateParent;
consensus.candidateWithoutParents = consensus.CandidateParent;
consensus.candidateHashDataOrdinary block:tonNode.blockIdExt collated_file_hash:int256 parent:consensus.CandidateParent = consensus.CandidateHashData;
consensus.candidateHashDataEmpty block:tonNode.blockIdExt parent:consensus.candidateId = consensus.CandidateHashData;
consensus.simplex.finalizeVote id:consensus.CandidateId = consensus.simplex.UnsignedVote;
