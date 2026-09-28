using System.ComponentModel;

namespace EdgeOptimizer.Settings.Core.Models;

public sealed class ProcessItem : INotifyPropertyChanged
{
    private bool _isSelected;
    private string _cpu;
    private string _memory;

    public ProcessItem(string name, string cpu, string memory, bool isSelected)
    {
        Name = name;
        _cpu = cpu;
        _memory = memory;
        _isSelected = isSelected;
    }

    public string Name { get; }
    public string Cpu => _cpu;
    public string Memory => _memory;

    public bool IsSelected
    {
        get => _isSelected;
        set
        {
            if (_isSelected == value)
            {
                return;
            }

            _isSelected = value;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(IsSelected)));
        }
    }

    /// <summary>Refresh live metrics in place so bound rows keep their position and focus.</summary>
    public void UpdateMetrics(string cpu, string memory)
    {
        if (_cpu != cpu)
        {
            _cpu = cpu;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Cpu)));
        }
        if (_memory != memory)
        {
            _memory = memory;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Memory)));
        }
    }

    public event PropertyChangedEventHandler? PropertyChanged;
}
