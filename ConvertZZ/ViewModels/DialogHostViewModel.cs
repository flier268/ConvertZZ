using System.Windows;
using System.Windows.Controls;
using PropertyChanged;

namespace ConvertZZ.ViewModels
{
    [AddINotifyPropertyChangedInterface]
    public class DialogHostViewModel
    {
        public bool MenuToggleButtonIsChecked { get; set; }
        public Visibility CreateShortcutVisibility { get; set; }
        public int ListBoxItemSelectedIndex { get; set; }
        public Page? Frame_Report { get; set; }
    }
}