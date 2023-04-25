// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/Context.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "abdk-libraries-solidity/ABDKMath64x64.sol";

interface MotoCoin {
  function setBalance(address accountAddress, uint256 amount) external;

  function transferFrom(
    address from,
    address to,
    uint256 amount
  ) external returns (bool);
}

contract MotoStaking is Context, Pausable, ReentrancyGuard, Ownable {
  modifier onlyAccount(address account) {
    require(_msgSender() == account, "only Account");
    _;
  }

  enum SnapshotChange {
    BALANCE,
    REWARD
  }

  // BalanceSnapshots are used to calculate rewrads based on the current pool
  // balance at the time of this snapshot's creation and duration. snapshots can only
  // be create/modified during funding periods.
  struct Snapshot {
    // the index as ID
    uint256 id;
    // current balance of pool at the time of snapshot creation
    uint256 balance;
    // block.timestamp of this snapshot's creation
    uint256 creationTime;
    // the number of reward and funding cycles that have passed since creation
    uint256 cyclesPassed;
    // the amount that is rewarded per cycle at the time of htis snapshot's creation
    uint256 rewardAmount;
  }

  // Account defines a Staking accountAddress
  struct Account {
    // block.timestamp of last account change
    uint256 unlockDate;
    // use to calculate lock out period
    uint256 lastStakeChangeTime;
    // the total staked amount
    uint256 stakedAmount;
    // the total reward amount that has yet to be sent to the accountAddress
    uint256 unpaidReward;
    // the toal reward paid out so far
    uint256 paidReward;
    // the last Snapshot of which a reward has been calculated
    //this field is also used to when to prune the SnapshotChain
    uint256 lastRewardCalculatedSnapshot;
  }

  MotoCoin public coin;
  uint256 private _currentSnapshotIndex;
  // minimal length of time in seconds that funds must be staked to earn a reward
  uint256 public lockDuration;
  // the period of time in which an Account is able to deposit or withdraw
  uint256 public fundDuration;
  // all snapshots. is prunable by calcReward function.
  uint256 public depositLockDuration;
  // only in the context where a snapshot is no longer needed to calculate a reward
  mapping(uint256 => Snapshot) public snapshots;
  // not prunable
  mapping(address => Account) public accounts;
  uint256 public _timeReference;

  constructor(
    address stakingCoin_,
    uint256 rewardAmount_,
    uint256 lockDuration_,
    uint256 fundDuration_,
    uint256 depositLockDurationMonths
  ) {
    //require(_isContract(stakingCoin_), "not contract");
    coin = MotoCoin(stakingCoin_);
    _currentSnapshotIndex = 0 ; 
    lockDuration = lockDuration_;
    fundDuration = fundDuration_;
    _timeReference = block.timestamp;
    snapshots[_currentSnapshotIndex] = Snapshot({
      id:_currentSnapshotIndex,
      balance:0,
      creationTime: block.timestamp,
      cyclesPassed:0,
      rewardAmount:rewardAmount_
    });
    depositLockDuration = 86400 * 30 * depositLockDurationMonths;
  }

  function getTotalStaked() public returns(uint256){
    return snapshots[_currentSnapshotIndex].balance;
  }

  //todo function to change cycleHours
  // todo function to change durationTime
  //deposit funds into the pool. updates snapshot
  function deposit(uint256 amount, address accountAddress) public {
    require(isFundingPeriod(), "Not currently funding period");
    Snapshot storage snapshot = snapshots[_currentSnapshotIndex];
    Account storage account = accounts[accountAddress];
    if(account.unlockDate == 0){
      uint256 unlockTime = block.timestamp + depositLockDuration;
      account.unlockDate = unlockTime;
    }
    if ((account.stakedAmount > 0) &&  (account.lastRewardCalculatedSnapshot != _currentSnapshotIndex)) {
      _calcReward(accountAddress);
    }
    coin.transferFrom(_msgSender(), address(this), amount);
    account.lastStakeChangeTime = block.timestamp;
    account.stakedAmount += amount;
    _updatePoolBalance(snapshot, (snapshot.balance + amount));
  }

  function getAccountInfo(address account) public returns (Account memory){
    return accounts[account];
  }

  function withdrawAll(address accountAddress)
    public
    onlyAccount(accountAddress)
  {
    Account storage account = accounts[accountAddress];
    require(account.stakedAmount > 0, "No stake found");
    require(isFundingPeriod(), "not currently funding period");
    require(account.unlockDate < block.timestamp,"funds still locked");
    _calcReward(accountAddress);
    coin.transferFrom(address(this), _msgSender(), account.stakedAmount);
    if (account.unpaidReward > 0) {
      coin.setBalance(accountAddress, account.unpaidReward);
      account.paidReward += account.unpaidReward;
      account.unpaidReward = 0;
    }
    Snapshot storage snapshot = snapshots[_currentSnapshotIndex];
     _updatePoolBalance(snapshot, (snapshot.balance - account.stakedAmount));
  }

  function withdrawReward(address accountAddress)
    public
    onlyAccount(accountAddress)
  {
    Account storage account = accounts[accountAddress];
    require(account.stakedAmount > 0, "No stake found");
    require(account.unpaidReward > 0, "No rewards generated");
    require(account.unlockDate < block.timestamp,"funds still locked");
    coin.setBalance(accountAddress, account.unpaidReward);
    account.paidReward += account.unpaidReward;
    account.unpaidReward = 0;
  }

  //todo: do off chain reward calculation
  /// updates pool balance according to deposit/withdraw
  function _updatePoolBalance(Snapshot storage snapshot, uint256 newBalance)
    private
  {
    uint256 thresholdTime = block.timestamp - fundDuration;
    if (thresholdTime < snapshot.creationTime) {
      snapshot.balance = newBalance;
    } else {
      uint256 cycles = _calcCycles(snapshot.creationTime);
      snapshot.cyclesPassed = cycles;
      _currentSnapshotIndex = _currentSnapshotIndex++;
      snapshots[_currentSnapshotIndex] = Snapshot({
        id: _currentSnapshotIndex,
        balance: newBalance,
        creationTime: block.timestamp,
        cyclesPassed: 0,
        rewardAmount: snapshot.rewardAmount
      });
    }
  }

  /**
  @notice on chain reward calculations.
   */
  function _calcReward(address accountAddress) private {
    Account storage account = accounts[accountAddress];
    require(account.stakedAmount > 0 , "No amount staked.");
    for (
      uint256 snapshotIndex = account.lastRewardCalculatedSnapshot;
      snapshotIndex <= _currentSnapshotIndex;
      snapshotIndex++
    ) {
      Snapshot memory snapshot = snapshots[snapshotIndex];
      int128 percent = ABDKMath64x64.divu(
        account.stakedAmount,
        snapshot.balance
      );
      uint256 rewardAmount = ABDKMath64x64.mulu(percent, snapshot.rewardAmount);
      account.unpaidReward += rewardAmount * snapshot.cyclesPassed;
    }
    Snapshot storage prevSnapshot = snapshots[
      account.lastRewardCalculatedSnapshot--
    ];
    if (prevSnapshot.id == 0) {
      delete snapshots[account.lastRewardCalculatedSnapshot];
    }
    account.lastRewardCalculatedSnapshot = _currentSnapshotIndex;
  }

  // time in hours
  function changeLockDuration(uint256 timeInHours) public onlyOwner {
    lockDuration = timeInHours * 3600;
    _iterateSnapshot();
  }

  function changeFundPeriod(uint256 timeInHours) public onlyOwner {
    fundDuration = timeInHours * 3600;
    _iterateSnapshot();
  }

  function changeDepositLock(uint256 months) public onlyOwner{
    depositLockDuration = 86400 * 30 * months;
  }

   function isFundingPeriod()
    public
    view
    returns (bool)
  {
    uint256 timePeriod = block.timestamp - _timeReference;
    //64.64 fixed point number
    int128 cycles = ABDKMath64x64.divu(
      timePeriod,
      (lockDuration + fundDuration)
    );
    //64 bit integer
    int64 cyclesInt = ABDKMath64x64.toInt(cycles);
    //64.64 fixed point number
    int128 remainder = cycles - int128(cyclesInt);
    uint256 value = ABDKMath64x64.mulu(
      remainder,
      (lockDuration + fundDuration)
    );
    return value <= lockDuration ;
  }


  function _iterateSnapshot(SnapshotChange change, uint256 value) private {
    Snapshot storage snapshot = snapshots[_currentSnapshotIndex];
    uint256 cycles = _calcCycles(snapshot.creationTime);
    snapshot.cyclesPassed = cycles;
    _currentSnapshotIndex = _currentSnapshotIndex++;
    if (change == SnapshotChange.BALANCE) {
       snapshots[_currentSnapshotIndex] = Snapshot({
      id: _currentSnapshotIndex,
      balance: value,
      creationTime: block.timestamp,
      cyclesPassed: 0,
      rewardAmount: snapshot.rewardAmount
    });
    } 
    else if (change == SnapshotChange.REWARD) {
       snapshots[_currentSnapshotIndex] = Snapshot({
      id: _currentSnapshotIndex,
      balance: snapshot.balance,
      creationTime: block.timestamp,
      cyclesPassed: 0,
      rewardAmount: value
    });
    }
   
  }

  function _iterateSnapshot() private {
    Snapshot storage snapshot = snapshots[_currentSnapshotIndex];
    uint256 cycles = _calcCycles(snapshot.creationTime);
    snapshot.cyclesPassed = cycles;
    _currentSnapshotIndex = _currentSnapshotIndex++;
    snapshots[_currentSnapshotIndex] = Snapshot({
      id: _currentSnapshotIndex,
      balance: snapshot.balance,
      creationTime: block.timestamp,
      cyclesPassed: 0,
      rewardAmount: snapshot.rewardAmount
    });
    _timeReference = block.timestamp;
  }

 

  function _calcCycles(uint256 startTime) private view returns (uint256) {
    uint256 timePeriod = block.timestamp - startTime;
    //64.64 fixed point number
    int128 cycles = ABDKMath64x64.divu(
      timePeriod,
      (lockDuration + fundDuration)
    );
    int64 cyclesInt = ABDKMath64x64.toInt(cycles);
    return uint256(uint64(cyclesInt));
  }

  function _removeSnapshot(uint256 index) private {
    Snapshot storage snapshot = snapshots[index--];
    if (snapshot.id == 0) {
      delete snapshots[index];
    }
  }

  function _isContract(address _addr) private view returns (bool) {
    uint32 size;
    assembly {
      size := extcodesize(_addr)
    }
    return (size > 0);
  }
}
