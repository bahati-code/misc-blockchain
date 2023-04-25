pragma solidity ^0.6.0; 

import "github.com/OpenZeppelin/openzeppelin-solidity/contracts/math/SafeMath.sol";

contract TimeReleasedFunds{
    //how to handle arbirtrary eth payments sent to contract?
    using SafeMath for uint256;
    address payable beneficiary;
    address creator;
    uint256 contractBalance;
    uint256 creationDate;
    uint256 releaseRatePercent = 1;
    uint256 installments = 0;
    uint256 installmentsSent = 0;
    uint256 installmentSize = 0;
    uint256 remainingPayments = 0;
    uint256 lastPaymentTime=0;
      
    event AdjustReleaseIntervals(address indexed beneficiary, uint256 releaseRatePercent, uint256 installmentSize,uint256 installments);
    event PaymentSent(address indexed beneficiary, uint256 amountSent, uint256 remainingPayments, uint256 time);
    
    constructor (address payable beneficiaryAddress) public payable{
        require(msg.value >=10 ether);
        creator  = msg.sender;
        beneficiary = beneficiaryAddress;
        contractBalance = uint256(msg.value);
        creationDate = now;
        releaseRatePercent = 1;
        (installmentSize,installments) =  calculateInstallments(releaseRatePercent);

    }
    
    
     modifier onlyCreator {
         require(msg.sender == creator);
         _;
     }
    
    
    function disbursePayment() public returns (bool){
        bool successStatus =  false;
        
        if(now >= (lastPaymentTime + 1 days)){
            if(contractBalance > installmentSize){
                lastPaymentTime = now;
                contractBalance.sub(installmentSize);
                beneficiary.transfer(installmentSize);

            }
            else if(contractBalance < installmentSize){
                beneficiary.transfer(address(this).balance);
            }
            successStatus = true;
            emit PaymentSent(beneficiary, installmentSize,contractBalance,now);
        }
        
        return successStatus;
    }
    
    
    function adjustReleaseRate() public onlyCreator returns (bool){
        releaseRatePercent = releaseRatePercent.add(1);
        (installmentSize,installments)  = calculateInstallments(releaseRatePercent);
        emit AdjustReleaseIntervals(beneficiary,releaseRatePercent,installmentSize,installments);
        return true;
    }
    
    
    function calculateInstallments(uint256 percent) private returns (uint256,uint256){
        assert (contractBalance > 0 && releaseRatePercent <= 99);
        uint256 defaultInstallmentNum = 100;
        return (contractBalance.div(defaultInstallmentNum.div(percent)),defaultInstallmentNum.div(percent));
        
    }
    
}