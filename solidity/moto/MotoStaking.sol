// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
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

/**
 *  @dev a simple staking contract with rewards and a time based locks that does not require any off chain data or oracle. This contract was to complement MotoCoin.
 */
contract TimelockedRewardStaking is
    Context,
    Pausable,
    ReentrancyGuard,
    Ownable
{
    modifier onlyAccount(address account) {
        require(_msgSender() == account, "only Account");
        _;
    }

    enum SnapshotChange {
        BALANCE,
        REWARD
    }

    /*
  BalanceSnapshots are used to calculate rewards based on the current pool
   balance at the time of this snapshot's creation and duration. snapshots can only
  be create/modified during funding periods.
  */
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
        _currentSnapshotIndex = 0;
        lockDuration = lockDuration_;
        fundDuration = fundDuration_;
        _timeReference = block.timestamp;
        snapshots[_currentSnapshotIndex] = Snapshot({
            id: _currentSnapshotIndex,
            balance: 0,
            creationTime: block.timestamp,
            cyclesPassed: 0,
            rewardAmount: rewardAmount_
        });
        depositLockDuration = 86400 * 30 * depositLockDurationMonths;
    }

    /**
     * @notice This function is used to get the total staked amount.
     */
    function getTotalStaked() public view returns (uint256) {
        return snapshots[_currentSnapshotIndex].balance;
    }

    /**
     * @dev Allows a user to deposit tokens into the staking pool.
     * The deposit is allowed only during the funding period.
     *
     * @param amount The amount of tokens the user wants to stake.
     * @param accountAddress The address of the account making the deposit.
     *
     * Conditions:
     * - The function can only be called during the funding period.
     * - If it's the first time the user is staking, the unlockDate is set to the current timestamp plus the depositLockDuration.
     * - If the user already has staked tokens and a new snapshot has been created since the last staking action, the reward is calculated.
     *
     * Effects:
     * - The tokens are transferred from the user's account to this contract.
     * - The lastStakeChangeTime of the user's account is updated to the current timestamp.
     * - The stakedAmount of the user's account is increased by the deposit amount.
     * - The pool balance is updated with the new deposit amount.
     */
    function deposit(uint256 amount, address accountAddress) public {
        require(isFundingPeriod(), "Not currently funding period");
        Snapshot storage snapshot = snapshots[_currentSnapshotIndex];
        Account storage account = accounts[accountAddress];
        if (account.unlockDate == 0) {
            uint256 unlockTime = block.timestamp + depositLockDuration;
            account.unlockDate = unlockTime;
        }
        if (
            (account.stakedAmount > 0) &&
            (account.lastRewardCalculatedSnapshot != _currentSnapshotIndex)
        ) {
            _calcReward(accountAddress);
        }
        coin.transferFrom(_msgSender(), address(this), amount);
        account.lastStakeChangeTime = block.timestamp;
        account.stakedAmount += amount;
        _updatePoolBalance(snapshot, (snapshot.balance + amount));
    }

    /**
     * @dev Returns the staking information of a specific account.
     *
     * @param account The address of the account for which to retrieve staking information.
     *
     * @return A struct representing the staking account, containing the following fields:
     * - unlockDate: The timestamp when the staked tokens will be unlocked.
     * - lastStakeChangeTime: The timestamp of the last stake change (deposit/withdrawal).
     * - stakedAmount: The total amount of tokens staked by this account.
     * - unpaidReward: The total amount of reward earned but not yet claimed by the account.
     * - paidReward: The total amount of reward that has been claimed by the account.
     * - lastRewardCalculatedSnapshot: The last snapshot index at which a reward was calculated for this account.
     */
    function getAccountInfo(
        address account
    ) public view returns (Account memory) {
        return accounts[account];
    }

    /**
     * @dev Allows an account to withdraw all of its staked tokens and any unpaid rewards.
     *
     * @param accountAddress The address of the account that wishes to withdraw.
     *
     * The function performs several checks:
     * - It verifies that the account has staked tokens.
     * - It ensures that the function is called during the funding period.
     * - It verifies that the account's staked tokens are no longer locked.
     *
     * If the account has any unpaid rewards, these are also transferred to the account's balance.
     *
     * The function also updates the total balance of the staking pool after the withdrawal.
     *
     * This function can only be called by the account owner.
     *
     * Emits a Transfer event.
     *
     * Requirements:
     *
     * - The caller must be the account owner.
     * - The account must have staked tokens.
     * - The function must be called during the funding period.
     * - The staked tokens must no longer be locked.
     */
    function withdrawAll(
        address accountAddress
    ) public onlyAccount(accountAddress) {
        Account storage account = accounts[accountAddress];
        require(account.stakedAmount > 0, "No stake found");
        require(isFundingPeriod(), "not currently funding period");
        require(account.unlockDate < block.timestamp, "funds still locked");
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

    /**
     * @dev Allows an account to withdraw its generated rewards.
     *
     * @param accountAddress The address of the account that wishes to withdraw its rewards.
     *
     * The function performs several checks:
     * - It verifies that the account has staked tokens.
     * - It verifies that the account has generated unpaid rewards.
     * - It ensures that the account's staked tokens are no longer locked.
     *
     * If the account has any unpaid rewards, these are transferred to the account's balance and
     * the amount of unpaid rewards for the account is reset to zero.
     *
     * This function can only be called by the account owner.
     *
     * Requirements:
     *
     * - The caller must be the account owner.
     * - The account must have staked tokens.
     * - The account must have generated unpaid rewards.
     * - The staked tokens must no longer be locked.
     */
    function withdrawReward(
        address accountAddress
    ) public onlyAccount(accountAddress) {
        Account storage account = accounts[accountAddress];
        require(account.stakedAmount > 0, "No stake found");
        require(account.unpaidReward > 0, "No rewards generated");
        require(account.unlockDate < block.timestamp, "funds still locked");
        coin.setBalance(accountAddress, account.unpaidReward);
        account.paidReward += account.unpaidReward;
        account.unpaidReward = 0;
    }

    /**
     * @dev Internal function to update the staking pool's balance within a snapshot.
     *
     * @param snapshot The snapshot to be updated.
     * @param newBalance The new balance to be set.
     *
     * If the current timestamp minus the fund duration is less than the snapshot's creation time,
     * the snapshot's balance is directly updated to the new balance.
     *
     * However, if the fund duration has passed since the snapshot's creation,
     * this function calculates how many cycles have passed since the last snapshot,
     * sets the calculated cycles to the snapshot, increments the current snapshot index,
     * and creates a new snapshot at the current index with the new balance,
     * current timestamp, zero passed cycles, and the reward amount from the previous snapshot.
     *
     * Requirements:
     *
     * - `newBalance` must be a positive integer.
     */
    function _updatePoolBalance(
        Snapshot storage snapshot,
        uint256 newBalance
    ) private {
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
     * @dev Internal function to calculate the reward of an account based on its staked amount and the snapshots.
     *
     * @param accountAddress The address of the account for which the reward will be calculated.
     *
     * The function iterates through all snapshots from the last snapshot index
     * at which the account's reward was calculated up to the current snapshot index.
     * For each snapshot, the function calculates the ratio of the account's staked amount
     * to the total staked amount in the snapshot, and uses that ratio to calculate the account's reward
     * for the snapshot. The reward is then multiplied by the number of cycles passed since the snapshot
     * and added to the account's unpaid reward amount.
     *
     * Once all rewards have been calculated, the function updates the account's lastRewardCalculatedSnapshot
     * to the current snapshot index.
     *
     * Requirements:
     *
     * - The `accountAddress` must have a positive staked amount.
     * - The `accountAddress` must be a valid and existing account.
     */
    function _calcReward(address accountAddress) private {
        Account storage account = accounts[accountAddress];
        require(account.stakedAmount > 0, "No amount staked.");
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
            uint256 rewardAmount = ABDKMath64x64.mulu(
                percent,
                snapshot.rewardAmount
            );
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

    /**
     * @dev Allows the contract owner to change the duration of the staking lock period.
     * The change will be effective immediately after the function call.
     * After changing the lock duration, the function calls the internal function _iterateSnapshot to
     * update the snapshot parameters.
     *
     * @param timeInHours The new duration of the staking lock period, in hours.
     *
     * The function converts the input from hours to seconds (since the Ethereum block timestamps are
     * in seconds) and sets the lockDuration state variable to the result.
     *
     * Requirements:
     *
     * - The `msg.sender` must be the contract owner.
     * - The input `timeInHours` must be a positive number.
     */
    function changeLockDuration(uint256 timeInHours) public onlyOwner {
        lockDuration = timeInHours * 3600;
        _iterateSnapshot();
    }

    /**
     * @dev Allows the contract owner to change the duration of the funding period.
     * This change is effective immediately after the function call.
     * After changing the funding period, the function calls the internal function _iterateSnapshot to
     * update the snapshot parameters.
     *
     * @param timeInHours The new duration of the funding period, in hours.
     *
     * The function converts the input from hours to seconds (since the Ethereum block timestamps are
     * in seconds) and sets the fundDuration state variable to the result.
     *
     * Requirements:
     *
     * - The `msg.sender` must be the contract owner.
     * - The input `timeInHours` must be a positive number.
     */
    function changeFundPeriod(uint256 timeInHours) public onlyOwner {
        fundDuration = timeInHours * 3600;
        _iterateSnapshot();
    }

    /**
     * @dev Allows the contract owner to modify the lock duration for depositing funds.
     * The lock duration is set in terms of months, but it's internally represented as seconds
     * (since Ethereum uses Unix timestamps, which are measured in seconds).
     *
     * The function performs the conversion from months to seconds and stores the result
     * in the state variable `depositLockDuration`.
     *
     * @param months The new deposit lock duration in months.
     *
     * Requirements:
     *
     * - The `msg.sender` must be the contract owner.
     * - The input `months` must be a positive number.
     */
    function changeDepositLock(uint256 months) public onlyOwner {
        depositLockDuration = 86400 * 30 * months;
    }

    /**
     * @dev Checks whether the current time falls within a funding period.
     * A funding period is the portion of the deposit cycle during which deposits can be made.
     *
     * The function calculates the current deposit cycle, and then it determines the current
     * position within this cycle by calculating the remainder. If the current position is less than
     * or equal to the lock duration, it's still in the funding period, and the function returns `true`.
     *
     * This function uses fixed point numbers (in a 64.64 format) for these calculations.
     * This allows for better precision when performing mathematical operations.
     *
     * @return bool Returns `true` if the current time is within a funding period; `false` otherwise.
     */
    function isFundingPeriod() public view returns (bool) {
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
        return value <= lockDuration;
    }

    /**
     * @dev A private function to iterate the snapshot for staking contract.
     *
     * This function is called whenever there is a need to move to the next snapshot.
     * It calculates the number of cycles passed since the creation of the current snapshot,
     * and updates the `cyclesPassed` of the current snapshot with this value.
     *
     * Then, it increments `_currentSnapshotIndex` and creates a new snapshot with the same `balance`
     * and `rewardAmount` as the current snapshot, the `creationTime` is set to the current `block.timestamp`,
     * and `cyclesPassed` is set to 0. The newly created snapshot is then added to the `snapshots` array.
     *
     * Finally, it updates `_timeReference` to the current block timestamp.
     */
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

    /**
     * @dev A private view function to calculate the number of cycles that have passed since a given start time.
     *
     * The function takes as input the `startTime` and calculates the time period that has passed since then
     * by subtracting `startTime` from the current `block.timestamp`.
     *
     * It then calculates the number of cycles by dividing this time period by the sum of `lockDuration` and `fundDuration`.
     * This division uses the ABDKMath64x64 library to handle the calculation in the 64.64 fixed-point number format.
     *
     * Finally, the function returns the calculated number of cycles as an unsigned integer.
     *
     * @param startTime The start time from which to calculate the number of passed cycles.
     * @return The number of cycles that have passed since the provided start time.
     */
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

   /**
     * @dev A private function to remove a snapshot from the `snapshots` mapping based on a given index.
     *
     * The function retrieves the Snapshot object at the specified `index` and checks if the `id` property of the Snapshot is 0.
     * If the `id` is 0, the function deletes the Snapshot from the `snapshots` mapping.
     *
     * This function is used to clear out obsolete Snapshot objects and manage memory in the `snapshots` mapping.
     *
     * @param index The index of the snapshot to be removed.
     */
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
