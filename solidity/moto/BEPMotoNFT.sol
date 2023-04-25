// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721URIStorage.sol";
import "./Adminable.sol";

contract BEPMotoNFT is ERC721URIStorage, Adminable {
    constructor(string memory _name, string memory _symbol)
        ERC721(_name, _symbol)
    {
        addMinter(owner());
        addAdmin(owner());
    }

    event NFTMinted(address indexed owner, uint256 indexed tokenID);
    event NFTBurned(address indexed burner, uint256 indexed tokenID);

    struct NFT {
        string name;
        uint256 chainId;
        address beneficiary;
        bytes32 contentHash;
        uint256 tokenId;
    }

    mapping(uint256 => NFT) private NFTmetaData;
    mapping(bytes32 => bool) private existingHash;
    mapping(uint256 => string) private idToHandle;
    mapping(string => uint256) private handleToId;

    //fees
    uint256 private _baseCreationFee;
    uint256 private _nameFee;
    uint256 private _handleFee;
    string private _payFeeWarningMsg = "pay fee";
    uint256 private _totalSupply = 0;

    //User Functions

    function userMint(
        string calldata name,
        uint256 chainId,
        address beneficiary,
        bytes32 contentHash,
        uint256 tokenId
    ) public payable {
        require(msg.value >= _baseCreationFee, _payFeeWarningMsg);
        NFT memory nft = NFT(name, chainId, beneficiary, contentHash, tokenId);
        _createNFT(nft);
    }

    function _createNFT(NFT memory nft) private {
        verifyUnique(nft);
        super._mint(nft.beneficiary, nft.tokenId);
        NFTmetaData[nft.tokenId] = nft;
        existingHash[nft.contentHash] = true;
        _totalSupply = _totalSupply++;
        emit NFTMinted(nft.beneficiary, nft.tokenId);
    }

    function verifyUnique(NFT memory nft) internal view {
        require(existingHash[nft.contentHash] == false, "file has nft");
        require(_exists(nft.tokenId) == false, "nft exists");
    }

    function verifyFingerprint(uint256 tokenId, bytes32 fingerprint)
        public
        view
        returns (bool)
    {
        NFT storage nft = NFTmetaData[tokenId];
        return nft.contentHash == fingerprint;
    }

    function existsAsNFT(bytes32 hash) public view returns (bool) {
        return existingHash[hash];
    }

    function burn(uint256 _tokenId) public {
        require(ownerOf(_tokenId) == _msgSender(), "only owner");
        super._burn(_tokenId);
        NFT storage nft = NFTmetaData[_tokenId];
        existingHash[nft.contentHash] = false;
        delete NFTmetaData[_tokenId];
        _totalSupply = _totalSupply--;
        emit NFTBurned(_msgSender(), _tokenId);
    }

    function nftFromTokenId(uint256 tokenID) public view returns (NFT memory) {
        return NFTmetaData[tokenID];
    }

    function nftFromHandle(string calldata handle)
        public view returns (NFT memory){
        require(handleToId[handle] != 0, "no handle");
        uint256 tokenId = handleToId[handle];
        return NFTmetaData[tokenId];
    }

    function changeName(uint256 tokenId, string calldata newName) public
        payable{
        require(msg.value >= _nameFee);
        require(ownerOf(tokenId) == _msgSender());
        _changeName(tokenId, newName);
    }

    function adminChangeName(uint256 tokenId, string calldata newName) public
    onlyAdmins{
      _changeName(tokenId,newName);
    }

    function _changeName(uint256 tokenId, string calldata newName) private {
        NFT storage nft = NFTmetaData[tokenId];
        nft.name = newName;
    }

    function changeHandle(uint256 tokenId, string calldata handle)
        public
        payable
    {
        require(msg.value >= _handleFee, _payFeeWarningMsg);
        require(ownerOf(tokenId) == _msgSender());
        _changeHandle(tokenId, handle);
    }

    function adminChangeHandle(uint256 tokenId, string calldata handle)
        public
        onlyAdmins
    {
        _changeHandle(tokenId, handle);
    }

    function _changeHandle(uint256 tokenId, string calldata handle) private {
        require(_exists(tokenId) == false, "no token");
        require(bytes(handle).length > 0, "value empty");
        require(handleToId[handle] == 0, "handle taken");
        handleToId[handle] = tokenId;
        idToHandle[tokenId] = handle;
    }

    function getHandle(uint256 tokenId) public view returns (string memory) {
        require(bytes(idToHandle[tokenId]).length != 0, "no handle");
        return idToHandle[tokenId];
    }

    function getCreationFee() public view returns (uint256) {
        return _baseCreationFee;
    }

    function getNameFee() public view returns (uint256) {
        return _nameFee;
    }

    function getHandleFee() public view returns (uint256) {
        return _handleFee;
    }

    //Admin Functions

    function adminMint(
        string calldata name,
        uint256 chainId,
        address beneficiary,
        bytes32 contentHash,
        uint256 tokenId
    ) public onlyMinters {
        NFT memory nft = NFT(name, chainId, beneficiary, contentHash, tokenId);
        _createNFT(nft);
    }

    function setTokenURI(uint256 _tokenId, string calldata _uri)
        public
        onlyAdmins
    {
        super._setTokenURI(_tokenId, _uri);
    }

    function setCreationFee(uint256 fee_) public onlyAdmins {
        _baseCreationFee = fee_;
        emit NewFeeSet("CreationFee", fee_, _msgSender());
    }

    function setNameFee(uint256 fee_) public onlyAdmins {
        _nameFee = fee_;
        emit NewFeeSet("NameFee", fee_, _msgSender());
    }

    function setHandleFee(uint256 fee_) public onlyAdmins {
        _handleFee = fee_;
        emit NewFeeSet("HandleFee", fee_, _msgSender());
    }

    function withdraw(uint256 amount) external onlyOwner {
        address payable payee = payable(owner());
        payee.transfer(amount);
    }

    function totalSupply() public view returns (uint256) {
        return _totalSupply;
    }
}
